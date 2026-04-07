import { Mark, mergeAttributes } from "@tiptap/core";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    issueMark: {
      setIssueMark: (attributes: Record<string, string>) => ReturnType;
    };
  }
}

const IssueMark = Mark.create({
  name: "issueMark",

  addAttributes() {
    return {
      issueId: { default: null },
      issueType: { default: null },
      severity: { default: null },
    };
  },

  parseHTML() {
    return [{ tag: "mark[data-issue-id]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "mark",
      mergeAttributes(HTMLAttributes, {
        class: `issue-mark issue-mark--${HTMLAttributes.severity ?? "low"}`,
        "data-issue-id": HTMLAttributes.issueId,
      }),
      0,
    ];
  },

  addCommands() {
    return {
      setIssueMark:
        (attributes) =>
        ({ commands }) =>
          commands.setMark(this.name, attributes),
    };
  },
});

export default IssueMark;
