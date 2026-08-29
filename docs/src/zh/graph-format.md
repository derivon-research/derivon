# 图格式

CLI 自有并直接接收 `derivon.graph/v1` graph 协议：

```json
{
  "points": [
    { "id": "A" },
    { "id": "B" }
  ],
  "hyperedges": [
    {
      "id": "h-ab",
      "weight": 1.5,
      "tails": ["A"],
      "head": "B"
    }
  ]
}
```

可选的顶层 schema 是 `"schema": "derivon.graph/v1"`。缺少 schema 时也严格表示 v1，而不是最新版。未知版本会被拒绝。mutation 和 subgraph 保留输入是否携带 schema，不做隐式升级或降级。

点与超边 ID 共用一个区分大小写的命名空间，并作为结构标识符由调用方分配。CLI 不从可选 `data` 推断身份，也不为其赋予应用语义。ID 长度为 1 到 128 个 ASCII 字节，并匹配：

```text
^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$
```

超边的 `tails` 不得重复，可以为空；`head` 必须引用已有点。weight 使用精确十进制，必须为十分之一的整数倍且不超过 `900719925474099.1`。科学计数法按数学值校验，负零输出为零；CLI 不经过 `f64` 或舍入。

`data` 可为任意 JSON 值且不参与验证与推导。data 中的 JSON number 保持任意精度，不经过二进制浮点数。所有位置（包括 data）出现重复对象键都会被拒绝。未修改的 `data` 保持 JSON 语义等价，但空白、键顺序、数字文本形式和 Unicode 转义形式可能变化。`data` 外的未知结构字段会被拒绝。

所有命令在执行前完整验证输入 graph；mutation 完成后再次完整验证。validation 确定性报告问题，不修复、排序或规范化输入。

点和超边数组保持输入顺序。add 追加，rename 原位修改，tails 保持传入顺序。普通结果集合按 ID 确定性排序，推导执行顺序不重新排序。输出对象键顺序确定；compact 不含多余空白，pretty 使用两空格。

## 合法边界

空图、孤立点、空尾超边、自依赖、环、平行超边、零权边和零权环都合法。超边可引用 points 数组中位于其后的点。ID 冲突、重复 ID/tail、未知引用、缺失字段、未知结构字段/schema、非法 weight 和重复 JSON key 都非法。

## 资源限制

默认 graph 输入上限为 256 MiB，operations/value 文件上限为 64 MiB，可以通过显式字节限制 flag 调高。JSON 嵌套深度固定为 128，不能关闭。不单独限制点或超边数量。
