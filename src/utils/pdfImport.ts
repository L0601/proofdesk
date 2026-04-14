import type {
  BlockLayout,
  NormalizedBlock,
  NormalizedDocument,
} from "@/types/models";

type PdfJsLib = typeof import("pdfjs-dist");

type TextItem = {
  str: string;
  transform: number[];
  width: number;
  height: number;
};

type Line = {
  y: number;
  items: TextItem[];
  text: string;
  avgHeight: number;
};

type ImportLogger = (message: string, payload?: unknown) => void;

type PdfImportOptions = {
  minBlockChars?: number;
};

const MIN_TEXT_ITEMS_PER_PAGE = 5;
const MIN_CHARS_PER_PAGE = 20;
const MAX_SUSPICIOUS_RATIO = 0.6;
const LINE_MERGE_Y_THRESHOLD = 3;
const PARAGRAPH_GAP_FACTOR = 1.65;

let pdfjsPromise: Promise<PdfJsLib> | null = null;

export async function extractPdfNormalizedDocument(
  source: string | Uint8Array,
  options: PdfImportOptions = {},
  logger?: ImportLogger,
): Promise<NormalizedDocument> {
  const minBlockChars = Math.max(0, Math.trunc(options.minBlockChars ?? 16));
  logger?.("开始加载 PDF 解析器", {
    sourceKind: typeof source === "string" ? "file_path" : "binary_data",
    minBlockChars,
  });
  const pdfjs = await loadPdfJs();
  logger?.("PDF 解析器已加载");
  const document = await pdfjs.getDocument(source).promise;
  logger?.("PDF 文档已打开", {
    numPages: document.numPages,
  });
  const suspiciousPages: number[] = [];
  const blocks: NormalizedBlock[] = [];
  let blockIndex = 0;

  for (let pageNumber = 1; pageNumber <= document.numPages; pageNumber += 1) {
    logger?.("开始提取页面文本", { pageNumber });
    const page = await document.getPage(pageNumber);
    const content = await page.getTextContent();
    const items = content.items.filter(isTextItem).map(normalizeTextItem);
    const totalChars = items.reduce((sum, item) => sum + item.str.length, 0);
    logger?.("页面文本提取完成", {
      pageNumber,
      itemCount: items.length,
      totalChars,
    });

    if (
      items.length < MIN_TEXT_ITEMS_PER_PAGE ||
      totalChars < MIN_CHARS_PER_PAGE
    ) {
      suspiciousPages.push(pageNumber);
    }

    const lines = buildLines(items);
    const paragraphs = buildParagraphs(lines);

    for (const paragraph of paragraphs) {
      const text = sanitizePdfParagraphText(paragraph.text);
      if (!text) {
        continue;
      }

      if (text.length < minBlockChars) {
        continue;
      }

      blocks.push({
        id: `blk_${String(blockIndex + 1).padStart(6, "0")}`,
        type: "paragraph",
        page: pageNumber,
        runs: [{ text, marks: [] }],
        text,
        layout: defaultLayout(),
        sourceMap: {
          sourceType: "pdf",
          page: pageNumber,
          itemRange: [paragraph.startItemIndex, paragraph.endItemIndex],
          locator: `page:${pageNumber}`,
        },
      });
      blockIndex += 1;
    }
  }

  if (
    document.numPages > 0 &&
    suspiciousPages.length / document.numPages >= MAX_SUSPICIOUS_RATIO
  ) {
    logger?.("PDF 被判定为疑似扫描件", {
      suspiciousPages,
      numPages: document.numPages,
    });
    throw new Error("该 PDF 可能为扫描件或图片型文档，当前版本暂不支持");
  }

  logger?.("PDF 标准化完成", {
    blockCount: blocks.length,
    suspiciousPages,
  });
  return {
    docId: "",
    sourceType: "pdf",
    version: 1,
    blocks,
  };
}

async function loadPdfJs() {
  if (!pdfjsPromise) {
    pdfjsPromise = import("pdfjs-dist").then((pdfjs) => {
      pdfjs.GlobalWorkerOptions.workerSrc = new URL(
        "pdfjs-dist/build/pdf.worker.mjs",
        import.meta.url,
      ).toString();
      return pdfjs;
    });
  }

  return pdfjsPromise;
}

function isTextItem(item: unknown): item is TextItem {
  return Boolean(
    item &&
      typeof item === "object" &&
      "str" in item &&
      "transform" in item &&
      Array.isArray((item as TextItem).transform),
  );
}

function normalizeTextItem(item: TextItem): TextItem {
  return {
    str: item.str ?? "",
    transform: item.transform,
    width: item.width ?? 0,
    height: item.height ?? 0,
  };
}

function buildLines(items: TextItem[]) {
  const sorted = [...items].sort((left, right) => {
    const yDelta = right.transform[5] - left.transform[5];
    if (Math.abs(yDelta) > LINE_MERGE_Y_THRESHOLD) {
      return yDelta;
    }
    return left.transform[4] - right.transform[4];
  });

  const lines: Line[] = [];
  for (const item of sorted) {
    const y = item.transform[5];
    const line = lines.find((current) => Math.abs(current.y - y) <= LINE_MERGE_Y_THRESHOLD);

    if (!line) {
      lines.push({
        y,
        items: [item],
        text: "",
        avgHeight: item.height || 12,
      });
      continue;
    }

    line.items.push(item);
    line.avgHeight =
      (line.avgHeight * (line.items.length - 1) + (item.height || 12)) /
      line.items.length;
  }

  return lines
    .map((line) => {
      const rowItems = [...line.items].sort(
        (left, right) => left.transform[4] - right.transform[4],
      );
      return {
        ...line,
        items: rowItems,
        text: joinLineText(rowItems),
      };
    })
    .filter((line) => line.text.trim().length > 0)
    .sort((left, right) => right.y - left.y);
}

function joinLineText(items: TextItem[]) {
  let text = "";

  for (let index = 0; index < items.length; index += 1) {
    const current = items[index];
    const previous = items[index - 1];

    if (!previous) {
      text += current.str;
      continue;
    }

    const previousRight = previous.transform[4] + previous.width;
    const gap = current.transform[4] - previousRight;
    const needsSpace =
      gap > Math.max(previous.height, current.height, 12) * 0.35 &&
      /[A-Za-z0-9]$/.test(previous.str) &&
      /^[A-Za-z0-9]/.test(current.str);

    text += `${needsSpace ? " " : ""}${current.str}`;
  }

  return text;
}

function buildParagraphs(lines: Line[]) {
  const paragraphs: Array<{
    text: string;
    startItemIndex: number;
    endItemIndex: number;
  }> = [];
  let itemCursor = 0;
  let current:
    | {
        textParts: string[];
        startItemIndex: number;
        endItemIndex: number;
        previousY: number;
        previousHeight: number;
      }
    | null = null;

  for (const line of lines) {
    const startItemIndex = itemCursor;
    const endItemIndex = itemCursor + line.items.length - 1;
    itemCursor += line.items.length;

    if (!current) {
      current = {
        textParts: [line.text],
        startItemIndex,
        endItemIndex,
        previousY: line.y,
        previousHeight: line.avgHeight,
      };
      continue;
    }

    const verticalGap = Math.abs(current.previousY - line.y);
    const shouldBreak = verticalGap > current.previousHeight * PARAGRAPH_GAP_FACTOR;

    if (shouldBreak) {
      paragraphs.push({
        text: current.textParts.join("\n"),
        startItemIndex: current.startItemIndex,
        endItemIndex: current.endItemIndex,
      });
      current = {
        textParts: [line.text],
        startItemIndex,
        endItemIndex,
        previousY: line.y,
        previousHeight: line.avgHeight,
      };
      continue;
    }

    current.textParts.push(line.text);
    current.endItemIndex = endItemIndex;
    current.previousY = line.y;
    current.previousHeight = line.avgHeight;
  }

  if (current) {
    paragraphs.push({
      text: current.textParts.join("\n"),
      startItemIndex: current.startItemIndex,
      endItemIndex: current.endItemIndex,
    });
  }

  return paragraphs;
}

function sanitizePdfParagraphText(text: string) {
  return text
    .replace(/^[\s\u3000\t]+/g, "")
    .replace(/[\s\u3000\t]+$/g, "")
    .replace(/\u3000+/g, " ")
    .replace(/\t+/g, " ")
    .replace(/[ \f\v]+/g, " ")
    .replace(/ *\n */g, "\n")
    .trim();
}

function defaultLayout(): BlockLayout {
  return {
    align: "left",
    indent: 0,
    lineBreakBefore: 0,
    lineBreakAfter: 1,
  };
}
