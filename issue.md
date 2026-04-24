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

## 2026-04-24 非正文 block 误入 AI 校对

- 现象：CIP、版权页、出版信息、目录等非正文内容被写入普通 block，后续会进入 AI 校对。
- 影响：这类内容不是正文，模型容易误报，既浪费调用次数，也会干扰问题列表。
- 当前优先处理范围：
  - `cip_colophon`
  - `toc`

### 目标

- 在 block 写入 `document_blocks` 时完成规则分类。
- 对高置信度非正文 block 打标，后续直接跳过 AI 校对。
- 规则写在代码中，文档中记录判定依据，后续可持续扩展。

### 首版分类结果

- `body`：正文，正常进入 AI 校对。
- `cip_colophon`：CIP、版权页、出版信息，跳过 AI 校对。
- `toc`：目录，跳过 AI 校对。

### 字段方案

- `content_role`：记录 block 的内容角色。
- `skip_proofread`：是否跳过 AI 校对。
- `skip_reason`：命中的规则原因，便于排查误判。

### 规则方案

#### 1. `cip_colophon`

命中以下强特征词时累计得分：

- `图书在版编目`
- `CIP`
- `中国国家版本馆`
- `ISBN`
- `出版发行`
- `责任编辑`
- `排版`
- `印刷`
- `开本`
- `印张`
- `字数`
- `版次`
- `印次`
- `定价`
- `版权所有`
- `翻印必究`
- `社址`
- `邮编`
- `电话`
- `网址`

判定策略：

- 命中 2 个及以上强特征词，可判为 `cip_colophon`
- 若同时出现 `CIP`、`ISBN`、`版次/印次` 这类组合，也可直接判定

#### 2. `toc`

目录 block 采用“逐行统计 + 占比阈值”判断。

目录项特征：

- 行首匹配 `第X章`、`第X节`
- 行首匹配 `参考文献`
- 行尾匹配页码，如 `1`、`42`、`143`
- 行内存在长串目录连接符或异常占位符

判定策略：

- 先按换行拆分文本
- 统计“像目录项的行”数量
- 当目录项行占比达到阈值时，判为 `toc`

### 校对阶段策略

- `skip_proofread = 1` 的 block 不进入 AI 校对队列。
- 这类 block 仍保留在 `document_blocks` 中，便于正文视图完整展示和后续排查。

### 风险与边界

- 首版仅处理高置信度整块，不做块内局部跳过。
- `参考文献正文`、`附录正文`、`索引` 等边界内容暂不自动跳过，避免误伤。
- 若目录与正文被错误切到同一 block，优先保守处理，不轻易跳过。

### 后续扩展方向

- 增加更多非正文类型：
  - `appendix`
  - `index`
  - `acknowledgement`
- 为规则增加得分和命中明细，便于调试。
- 结合版式信息或页位置，提高目录和版权页识别精度。
