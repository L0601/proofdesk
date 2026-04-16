export type SourceType = "docx" | "pdf";

export type ProjectStatus =
  | "draft"
  | "ready"
  | "processing"
  | "completed"
  | "failed";

export type BlockType = "paragraph" | "heading" | "table_cell";

export type ProofreadingStatus =
  | "pending"
  | "running"
  | "paused"
  | "completed"
  | "failed";

export type ProofreadingMode = "full" | "retry_failed" | "selection";

export type IssueType =
  | "typo"
  | "punctuation"
  | "grammar"
  | "wording"
  | "redundancy"
  | "consistency";

export type IssueSeverity = "low" | "medium" | "high";

export type IssueStatus = "open" | "accepted" | "ignored" | "resolved";

export type TextMark =
  | "bold"
  | "italic"
  | "underline"
  | "strike"
  | "superscript"
  | "subscript";

export interface ProjectSummary {
  id: string;
  name: string;
  sourceType: SourceType;
  sourceFileName: string;
  status: ProjectStatus;
  totalBlocks: number;
  completedBlocks: number;
  failedBlocks: number;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectDetail extends ProjectSummary {
  sourceFilePath: string;
  normalizedDocPath: string;
}

export interface BlockRun {
  text: string;
  marks: TextMark[];
}

export interface BlockLayout {
  align: "left" | "center" | "right" | "justify";
  indent: number;
  lineBreakBefore: number;
  lineBreakAfter: number;
}

export interface SourceMap {
  sourceType: SourceType;
  paragraphIndex?: number | null;
  runRange?: [number, number] | null;
  page?: number | null;
  itemRange?: [number, number] | null;
  locator?: string | null;
}

export interface NormalizedBlock {
  id: string;
  type: BlockType;
  page: number | null;
  runs: BlockRun[];
  text: string;
  layout: BlockLayout;
  sourceMap: SourceMap;
}

export interface NormalizedDocument {
  docId: string;
  sourceType: SourceType;
  version: number;
  blocks: NormalizedBlock[];
}

export interface DocumentBlock {
  id: string;
  projectId: string;
  blockIndex: number;
  type: BlockType;
  textContent: string;
  jsonPayload: string;
  sourcePage: number | null;
  sourceLocator: string | null;
  charCount: number;
  proofreadingStatus: ProofreadingStatus;
  updatedAt: string;
}

export interface ProofreadingJob {
  id: string;
  projectId: string;
  mode: ProofreadingMode;
  status: ProofreadingStatus;
  optionsJson?: string | null;
  autoResume?: boolean;
  startedAt: string | null;
  finishedAt: string | null;
  totalBlocks: number;
  completedBlocks: number;
  failedBlocks: number;
  totalIssues: number;
  totalTokensIn: number;
  totalTokensOut: number;
  totalLatencyMs: number;
}

export interface ProofreadingCall {
  id: string;
  jobId: string;
  projectId: string;
  blockId: string;
  modelName: string;
  baseUrl: string;
  requestJson: string;
  responseJson: string | null;
  status: string;
  startedAt: string;
  finishedAt: string | null;
  latencyMs: number | null;
  promptTokens: number | null;
  completionTokens: number | null;
  errorMessage: string | null;
}

export interface ProofreadingIssue {
  id: string;
  projectId: string;
  jobId: string;
  blockId: string;
  issueType: IssueType;
  severity: IssueSeverity;
  startOffset: number;
  endOffset: number;
  quoteText: string;
  prefixText: string | null;
  suffixText: string | null;
  suggestion: string;
  explanation: string;
  normalizedReplacement: string | null;
  status: IssueStatus;
  createdAt: string;
}

export interface AppSettings {
  baseUrl: string;
  apiKey: string;
  model: string;
  timeoutMs: number;
  maxConcurrency: number;
  pdfMinBlockChars: number;
  proofreadSkipPages: number;
  temperature: number;
  maxTokens: number;
  systemPromptTemplate: string;
}

export interface ProofreadOptions {
  mode: ProofreadingMode;
  maxChunkChars: number;
  overlapChars: number;
  issueTypes: IssueType[];
}
