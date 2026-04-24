# Issue 记录

用于记录导入、解析、校对流程中的问题，后续可持续追加。

## 2026-04-24 PDF 文本层重叠导致 block 文本重复

- 现象：`blk_000387` 出现标题重复，内容变成两句连在一起。
- 项目：`4b44d9db-1022-4b8d-8d94-584b434a5343`
- 页码：第 82 页
- 对应文本：`4. 融合 IoT 与健康管理模型，推动服务模式向“以健康为中心”转变`

### 排查结论

- 不是数据库重复写入。
- 不是跨页 block 合并导致。
- 原始 PDF 第 82 页文本层中，这一行标题存在两份完全重叠的 text item。
- 视觉上只显示一行，但 `pdfjs` 抽取时会读出两次。

### 复现依据

- 直接读取原始 PDF 第 82 页 text items，得到两条完全相同的记录。
- 两条记录的 `str`、`x`、`y`、`w`、`h` 一致。

### 排查命令

```sh
node --input-type=module -e '
import * as pdfjs from "pdfjs-dist/legacy/build/pdf.mjs";
const path = "/Users/ltc/Library/Application Support/com.yanzhun.proofdesk/projects/4b44d9db-1022-4b8d-8d94-584b434a5343/original/source.pdf";
const doc = await pdfjs.getDocument(path).promise;
const page = await doc.getPage(82);
const content = await page.getTextContent();
const items = content.items.filter(x => x && typeof x.str === "string");
for (const [i, item] of items.entries()) {
  if (item.str.includes("融合 IoT") || item.str.includes("以健康为中心")) {
    console.log(JSON.stringify({
      index: i,
      str: item.str,
      x: item.transform?.[4],
      y: item.transform?.[5],
      w: item.width,
      h: item.height
    }));
  }
}
'
```

### 命令输出

```text
{"index":21,"str":"4. 融合 IoT 与健康管理模型，推动服务模式向“以健康为中心”转变","x":86.1958,"y":532.7485,"w":330.1716,"h":11}
{"index":22,"str":"4. 融合 IoT 与健康管理模型，推动服务模式向“以健康为中心”转变","x":86.1958,"y":532.7485,"w":330.1716,"h":11}
```

### 命令结论

1. 视觉上只显示一行，是因为两份文字完全重叠。
2. 抽取时 `pdfjs` 把两份都读出来了。
3. `blk_000387` 里的重复，是 PDF 文本层重复导致的，不是后面的 block 存储或跨页合并问题。

### 修复方案

- 在 `src/utils/pdfImport.ts` 中，`getTextContent()` 之后、`buildLines()` 之前增加重叠文本去重。
- 去重条件：
  - 文本内容完全一致
  - `x/y` 坐标在极小容差内
  - `width/height` 在极小容差内
- 满足条件时仅保留一份，避免进入后续分行和分段流程。

### 风险说明

- 该规则只处理“同位置同文本”的重叠项。
- 正常排版中的真实重复句子，只要位置不同，不会被误删。

