import type {
  BlockType,
  NormalizedBlock,
  NormalizedDocument,
  TextMark,
} from "@/types/models";

type TiptapMark = {
  type: "bold" | "italic" | "underline" | "strike";
};

type TiptapTextNode = {
  type: "text";
  text: string;
  marks?: TiptapMark[];
};

type TiptapBlockNode = {
  type: "paragraph" | "heading";
  attrs: {
    blockId: string;
    sourcePage: number | null;
    level?: number;
  };
  content: TiptapTextNode[];
};

export function buildEditorDoc(document: NormalizedDocument) {
  return {
    type: "doc",
    content: document.blocks.map(toEditorBlock),
  };
}

function toEditorBlock(block: NormalizedBlock): TiptapBlockNode {
  const content =
    block.runs.length > 0
      ? block.runs.map((run) => ({
          type: "text" as const,
          text: run.text,
          marks: toEditorMarks(run.marks),
        }))
      : [{ type: "text" as const, text: "" }];

  if (block.type === "heading") {
    return {
      type: "heading",
      attrs: {
        blockId: block.id,
        sourcePage: block.page,
        level: inferHeadingLevel(block.type),
      },
      content,
    };
  }

  return {
    type: "paragraph",
    attrs: {
      blockId: block.id,
      sourcePage: block.page,
    },
    content,
  };
}

function toEditorMarks(marks: TextMark[]): TiptapMark[] | undefined {
  const mapped = marks
    .map((mark) => {
      switch (mark) {
        case "bold":
          return { type: "bold" as const };
        case "italic":
          return { type: "italic" as const };
        case "underline":
          return { type: "underline" as const };
        case "strike":
          return { type: "strike" as const };
        default:
          return null;
      }
    })
    .filter((mark): mark is TiptapMark => Boolean(mark));

  return mapped.length ? mapped : undefined;
}

function inferHeadingLevel(blockType: BlockType) {
  return blockType === "heading" ? 2 : 1;
}
