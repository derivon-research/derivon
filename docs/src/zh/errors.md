# 错误

stdout 只用于成功 JSON；错误以结构化 JSON 写入 stderr。失败 mutation 不输出 graph。message 仅用于诊断，不是稳定机器契约；脚本使用 code、issue code、path 和 details。

## 退出码与顶层 Code

| 退出码 | 稳定顶层 code |
| ---: | --- |
| 0 | 成功，包括不可达和 route 预算耗尽 |
| 64 | `invalid_arguments` |
| 65 | `invalid_json`、`duplicate_key`、`input_limit_exceeded`、`nesting_limit_exceeded`、`unsupported_schema`、`invalid_graph`、`invalid_operations`、`invalid_id`、`invalid_weight`、`unknown_point`、`unknown_hyperedge`、`id_conflict`、`point_referenced`、`invalid_pointer`、`pointer_not_found`、`pointer_type_mismatch` |
| 66 | `file_not_found`、`file_unreadable` |
| 70 | `internal` |
| 74 | `io` |

完整 graph validation 使用 `invalid_graph` 和 issues；apply 结构或 operation 失败使用 `invalid_operations`；单条 CLI mutation 使用具体业务 code。details 可携带 id、source、limit 等字段。

稳定 issue code 为：`missing_field`、`unknown_field`、`invalid_type`、`invalid_id`、`duplicate_id`、`duplicate_tail`、`unknown_point`、`invalid_weight`、`duplicate_key`、`invalid_pointer`、`pointer_not_found`、`pointer_type_mismatch`、`point_referenced`。

无法继续的 parser 错误不制造后续 issue。其余 issue 依次按 schema/顶层、points 数组、hyperedges 数组、cross-reference 排列；实体字段使用文档顺序，apply 使用 operation 数组及字段顺序。同一路径按 issue code 排序。无效父值不产生级联子错误。

不可达和 route 预算耗尽不是错误。最终 stdout transport 失败退出 74，OS 已接收的字节无法撤回。
