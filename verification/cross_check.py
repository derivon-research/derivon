"""
跨语言对拍：Rust 实现 vs Python 穷举预言机。

用法：
    cargo run -q -p derivon-core --example selftest -- --count 400 --seed 7 > /tmp/rust.jsonl
    python3 verification/cross_check.py /tmp/rust.jsonl

## 为什么需要跨语言

Rust 侧已经有一个穷举与分支定界对拍，那能抓住剪枝、记忆化、分支遗漏这类实现错误。
但它抓不住**对模型本身的共同误解**——两份实现出自同一作者、同一次会话、同一遍对规范
的阅读；更要命的是，那个穷举调用的是**与求解器同一个** closure_restricted，所以该函数
里的任何错误对它完全隐形：两边一致，测试全绿。

本文件的预言机是独立的：另一次实现、另一种语言、自己的可达性代码，且已在 1325 组查询
上与 verify_v6 的穷举交叉验证过。这是唯一能发现"规范被读错了"的检查。

## 关键设计：不信任 Rust 给的任何数字

每条记录里的 cost / derivation 都只当作**待检验的断言**，而不是输入：

  - 成本由 Python 自己的 d_set_bruteforce 从实例重算
  - Rust 返回的 derivation 由 **Python 自己的 closure** 重新验证能否推出目标

第二条是重点。它把 Rust 的 closure_restricted 置于独立审视之下——如果它多算或少算了
可达性，Python 这边会立刻不同意。这正是 Rust 内部那个穷举做不到的事。

## 一处已知的语义差异，不要当成 bug

Rust 在证明最优后会把 lower 收紧到实际最优值；Python 的 Solution 始终保留初始的
d_depth。两者都对，Rust 的信息量更大。因此本脚本**不比对 lower 与 Python 的 d_depth**，
只检查区间自洽（lower <= cost <= upper）。同理 derivation 也不按相等比较：存在多个等价
最优解时选哪个取决于遍历顺序，两侧内部编号又不同，所以只验证它自洽。
"""

import json
import sys
from collections import Counter

from verify_v6 import closure, d_set_bruteforce, INF


def load(record):
    """把一行 JSON 还原成 verify_v6 的表示：(P, H, S, t, 边名到下标)。"""
    points = set(record["points"])
    edges = []
    index_of = {}
    for i, edge in enumerate(record["edges"]):
        index_of[edge["name"]] = i
        edges.append((set(edge["tail"]), edge["head"], float(edge["weight"])))
    return points, edges, set(record["start"]), record["target"], index_of


def check(record, line_no):
    """返回失败原因列表；空列表表示通过。"""
    P, H, S, t, index_of = load(record)
    failures = []

    rust_cost = record["cost"]
    rust_cost = INF if rust_cost is None else float(rust_cost)

    # 1) 可达性：Python 自己算一遍，必须和 Rust 是否给出有限成本一致。
    #    这条是当初抓出「空尾超边被空队列吞掉」那个 bug 的判据。
    reachable_here = t in closure(P, H, S)
    if reachable_here != (rust_cost < INF):
        failures.append(
            f"可达性不一致：Python={reachable_here} Rust={'可达' if rust_cost < INF else '不可达'}"
        )
        return failures  # 后面的比较失去意义

    if not reachable_here:
        if record["derivation"]:
            failures.append("目标不可达，但 Rust 返回了非空推导")
        return failures

    # 2) 成本：Python 用穷举独立重算。
    #    分两种情况——Rust 声称证明了最优，就必须逐位相同；只给出区间（预算耗尽），
    #    则只要求精确解确实落在它承诺的区间内。后者不是放水：区间是它对外的承诺，
    #    承诺被违反同样是 bug。
    exact = d_set_bruteforce(P, H, S, t)
    proven = record.get("proven_optimal", True)
    if proven:
        if abs(exact - rust_cost) > 1e-9:
            failures.append(f"声称最优但成本不一致：Python 穷举={exact} Rust={rust_cost}")
    else:
        lo = INF if record["lower"] is None else float(record["lower"])
        hi = INF if record["upper"] is None else float(record["upper"])
        if not (lo - 1e-9 <= exact <= hi + 1e-9):
            failures.append(f"未收敛，但精确解 {exact} 落在承诺区间 [{lo}, {hi}] 之外")

    # 3) Rust 的推导，用 Python 自己的 closure 重新验证。
    #    这一步让 Rust 的 closure_restricted 受到独立检验。
    allowed = set()
    for name in record["derivation"]:
        if name not in index_of:
            failures.append(f"推导里出现了不存在的边名：{name}")
            return failures
        allowed.add(index_of[name])

    if t not in closure(P, H, S, allowed=allowed):
        failures.append("Rust 返回的推导，按 Python 的可达性算不出目标")

    # 4) 推导的权和必须等于它自称的成本。
    summed = sum(H[i][2] for i in allowed)
    if abs(summed - rust_cost) > 1e-9:
        failures.append(f"推导权和={summed} 与自称成本={rust_cost} 不符")

    # 5) 区间自洽。注意只查这个，不与 Python 的 d_depth 比较（见文件头说明）。
    lower = record["lower"]
    upper = record["upper"]
    if lower is None or upper is None:
        failures.append("目标可达但区间端点为 null")
    elif not (float(lower) <= rust_cost + 1e-9 <= float(upper) + 1e-9):
        failures.append(f"区间不自洽：lower={lower} cost={rust_cost} upper={upper}")

    return failures


def main(path):
    total = 0
    failed = 0
    summary = None
    coverage = Counter()

    with open(path, encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            record = json.loads(line)
            if "_summary" in record:
                summary = record["_summary"]
                continue
            total += 1

            # 覆盖面统计。一批不含空尾 / 空起点 / 不可达的实例，其实什么都没测到。
            if any(not e["tail"] for e in record["edges"]):
                coverage["含空尾超边"] += 1
            if not record["start"]:
                coverage["空起点集"] += 1
            if record["cost"] is None:
                coverage["目标不可达"] += 1
            if any(len(e["tail"]) > 1 for e in record["edges"]):
                coverage["含多前提超边"] += 1
            if not record.get("proven_optimal", True):
                coverage["未证明最优"] += 1

            problems = check(record, line_no)
            if problems:
                failed += 1
                print(f"✗ 第 {line_no} 行")
                for p in problems:
                    print(f"    {p}")
                if failed <= 3:  # 前几个失败打出实例，便于用同一 seed 复现
                    print(f"    实例：{line[:400]}")

    # 没有结尾行 = dump 中途夭折（Rust 侧 panic）。这种情况下前面那些"通过"毫无意义，
    # 因为批次被悄悄截短了。必须当作失败，否则一次 panic 会伪装成一次成功。
    if summary is None:
        print("✗ 缺少结尾汇总行：Rust 侧的 dump 中途中断了，本次结果不可信")
        return 1
    if summary["count"] != total:
        print(f"✗ 实例数不符：Rust 声称生成 {summary['count']} 组，实际收到 {total} 组")
        return 1

    print(f"\n对拍 {total} 组，失败 {failed} 组（seed={summary['seed']}）")
    print("覆盖面：")
    for name, count in sorted(coverage.items()):
        print(f"    {name:12} {count:4} 组 ({count / total:.0%})")
    for required in ("含空尾超边", "空起点集", "目标不可达", "含多前提超边"):
        if coverage[required] == 0:
            print(f"⚠ 这批实例不含「{required}」，覆盖面不足")
            failed += 1

    return 1 if failed else 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(__doc__)
        sys.exit(2)
    sys.exit(main(sys.argv[1]))
