import type {
  BlockType,
  NormalizedBlock,
  NormalizedDocument,
  ProofreadingIssue,
  TextMark,
} from "@/types/models";

type TiptapMark =
  | { type: "bold" | "italic" | "underline" | "strike" }
  | {
      type: "issueMark";
      attrs: {
        issueId: string;
        issueType: string;
        severity: string;
      };
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

type Segment = {
  text: string;
  marks: TiptapMark[];
};

export function buildEditorDoc(
  document: NormalizedDocument,
  issues: ProofreadingIssue[] = [],
) {
  const issueMap = groupIssuesByBlock(issues);

  return {
    type: "doc",
    content: document.blocks.map((block) =>
      toEditorBlock(block, issueMap.get(block.id) ?? []),
    ),
  };
}

function toEditorBlock(
  block: NormalizedBlock,
  issues: ProofreadingIssue[],
): TiptapBlockNode {
  const content = buildBlockContent(block, issues);

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

function buildBlockContent(
  block: NormalizedBlock,
  issues: ProofreadingIssue[],
): TiptapTextNode[] {
  const issueRanges = issues
    .map((issue) => ({
      start: issue.startOffset,
      end: issue.endOffset,
      mark: {
        type: "issueMark" as const,
        attrs: {
          issueId: issue.id,
          issueType: issue.issueType,
          severity: issue.severity,
        },
      },
    }))
    .sort((left, right) => left.start - right.start);

  const nodes: TiptapTextNode[] = [];
  let cursor = 0;

  for (const run of block.runs) {
    const runStart = cursor;
    const runEnd = cursor + run.text.length;
    const baseMarks = toTextMarks(run.marks);
    const segments: Segment[] = [{ text: run.text, marks: baseMarks }];

    for (const issue of issueRanges) {
      if (issue.end <= runStart || issue.start >= runEnd) {
        continue;
      }

      const relativeStart = Math.max(issue.start - runStart, 0);
      const relativeEnd = Math.min(issue.end - runStart, run.text.length);
      splitSegments(segments, relativeStart, relativeEnd, issue.mark);
    }

    for (const segment of segments) {
      if (!segment.text) {
        continue;
      }

      nodes.push({
        type: "text",
        text: segment.text,
        marks: segment.marks.length ? segment.marks : undefined,
      });
    }

    cursor = runEnd;
  }

  return nodes.length ? nodes : [{ type: "text", text: "" }];
}

function splitSegments(
  segments: Segment[],
  start: number,
  end: number,
  issueMark: Extract<TiptapMark, { type: "issueMark" }>,
) {
  let offset = 0;

  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    const segmentStart = offset;
    const segmentEnd = offset + segment.text.length;
    offset = segmentEnd;

    if (end <= segmentStart || start >= segmentEnd) {
      continue;
    }

    const localStart = Math.max(start - segmentStart, 0);
    const localEnd = Math.min(end - segmentStart, segment.text.length);
    const replacement: Segment[] = [];

    if (localStart > 0) {
      replacement.push({
        text: segment.text.slice(0, localStart),
        marks: [...segment.marks],
      });
    }

    const middleMarks = hasIssueMark(segment.marks, issueMark.attrs.issueId)
      ? [...segment.marks]
      : [...segment.marks, issueMark];

    replacement.push({
      text: segment.text.slice(localStart, localEnd),
      marks: middleMarks,
    });

    if (localEnd < segment.text.length) {
      replacement.push({
        text: segment.text.slice(localEnd),
        marks: [...segment.marks],
      });
    }

    segments.splice(index, 1, ...replacement);
    index += replacement.length - 1;
  }
}

function hasIssueMark(marks: TiptapMark[], issueId: string) {
  return marks.some(
    (mark) =>
      mark.type === "issueMark" && mark.attrs.issueId === issueId,
  );
}

function toTextMarks(marks: TextMark[]): TiptapMark[] {
  return marks
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
    .filter((mark): mark is Extract<TiptapMark, { type: "bold" | "italic" | "underline" | "strike" }> => Boolean(mark));
}

function groupIssuesByBlock(issues: ProofreadingIssue[]) {
  const map = new Map<string, ProofreadingIssue[]>();

  for (const issue of issues) {
    const current = map.get(issue.blockId) ?? [];
    current.push(issue);
    map.set(issue.blockId, current);
  }

  return map;
}

function inferHeadingLevel(blockType: BlockType) {
  return blockType === "heading" ? 2 : 1;
}
