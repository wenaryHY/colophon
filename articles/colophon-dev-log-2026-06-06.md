# Colophon 开发日志：Action Registry、Webhook 加固与 89 个测试——一个无头 CMS 的六月精修

**2026-06-06 · 技术复盘**

---

## 开篇

上篇文章里我说"下个月见"。flag 立完第二天就开始写代码了——这次不是大功能，而是一轮"生产就绪"级别的精修。

Colophon 跑在 DigitalOcean 1C2G 土豆服务器上已经一个多月了。表面上风平浪静——CPU 3%，内存 20MB。但我知道底下藏着多少"假设它能跑"的定时炸弹。这个月的主题就是：**把每一个不可观测的角落点亮**。

---

## 1. Action Registry：从黑洞到可追踪

插件系统的 Action Hook 一直有个尴尬的问题——`tokio::spawn` 之后，`JoinHandle` 被直接丢弃。Action 执行成功？失败？超时？panic 了？统统不知道。Fire-and-forget 变成了 fire-and-pray。

于是做了 **Action Registry**——一个全局单例，在内存中追踪每一个 spawned action 的完整生命周期：

```
action spawned  → running  → done / failed / timeout
```

每个 action 分配一个 UUID，带 `started_at` 和 `finished_at` 两个时间戳，失败时记录完整错误信息。状态机用 tracing 日志输出到 systemd journal，查看只需一行：

```bash
journalctl -u colophon | grep action_registry
```

`info` 级别记录 spawn/done，`error` 记录失败，`warn` 记录超时。没有前端页面——这不是给用户看的，是给运维和调试用的。超过 1 小时的已完成记录会被惰性清理，不会撑爆内存。

做完之后，任何一个插件 Hook 的异常都不会再静默消失。这才是生产环境该有的样子。

---

## 2. Slug 冲突后缀：随机六位，而非递增数字

Slug 冲突时的回退策略一直存在，但实现比较粗暴——如果 `my-slug` 被占用了，追加个什么后缀？之前的方案是递增后缀，但这有几个问题：暴露文章数量、可预测、容易被爬虫穷举。

现在改成了**随机 6 位 hex 后缀**（从 UUID 截取前 6 个字符）：

```
my-slug        → 被占用
my-slug-a3f9b2 → 随机后缀，查一次，不冲突就用这个
```

最多尝试 10 次随机后缀。如果 10 次都冲突，fallback 到完整 UUID：`my-slug-550e8400-e29b-41d4-a716-446655440000`。

测试覆盖了四个场景：无冲突直接用、冲突追加随机后缀、已删除文章的 slug 也算占用、排除自身 ID 时不冲突。4 个 slug 专项测试，全部绿灯。

---

## 3. Webhook 三重加固：并发、超时、哨兵记录

这是本轮改动最大的模块。先说背景——Webhook **本来就有指数退避重试**：延迟公式 `5 × 2^(n-1)` 秒，序列为 5s → 10s → 20s → 40s → 60s，最多 5 次重试，4xx 客户端错误不重试。这个机制一直没变。

这次改的是三个新东西：

**（a）Semaphore(5) 有界并发。** 之前 webhook 串行发送——12 个 webhook 挨个调，一个慢就全堵。现在是 `tokio::sync::Semaphore(5)` 控制，最多同时发出 5 个 HTTP 请求。第 6 个等前面的释放 permit 再发。并发数可通过 `webhook.max_concurrency` 配置。

**（b）60 秒总超时。** `tokio::time::timeout` 包住 `join_all`——如果整批 webhook 在 60 秒内没全部完成，未完成的直接取消。超时秒数可通过 `webhook.timeout_seconds` 配置。

**（c）DB 查询失败不静默丢弃。** 这是最重要的修复。之前的代码在查询 webhook 列表这一步如果 DB 挂了，`tracing::error!` 打完日志就直接 `return`——该事件的所有 webhook 调用静默丢失，没有任何记录。现在改为插入一个 `__event_failed__` 哨兵记录到 `webhook_deliveries` 表，包含事件名、payload 和错误信息。哨兵 webhook 的 `enabled=0`，不会被正常分发匹配到，仅用于满足外键约束。这样即使 DB 临时不可用，事件也不会在日志之外完全蒸发。

---

## 4. 移动端 IME 测试：17 项，三平台，零 Windows

移动端编辑器最怕的不是布局错乱，是输入法。Tiptap 底层的 ProseMirror 在处理 `compositionstart` / `compositionend` 事件时是社区长期反馈的高频痛点区——候选词消失、字符重复、光标跳位、退格异常。

于是写了一份 **17 项 IME 兼容性测试清单**，覆盖三个平台：

| 平台 | 输入法 |
|---|---|
| iOS Safari | 中文拼音 |
| Android Chrome | Gboard |
| Android Chrome | 搜狗输入法 |

测试项覆盖了：标题框中英文输入、编辑器长文输入、拼音中途退格、换行、加粗、链接插入、复制粘贴、撤销、源码模式切换、快速连打、长按选字、500 字长文、光标跳转、emoji 输入、app 切换后恢复。

没有 Windows——桌面端浏览器不存在 IME composition 的兼容性问题。也没有语音输入——语音转文字最终提交的是 committed 文本，不经过 composition 管道。

---

## 5. 测试：89 个新增

自动化工具一次性加了 **85 个单元测试**，覆盖了 10 个之前零测试覆盖的模块——Auth、Post、Comment、Media 等核心域。加上后来补的 4 个 slug 冲突解析测试，一共 **89 个新增测试**。

不是"测试覆盖率达到 90%"，而是"新增了 89 个测试"。这两句话的区别很大——前者是覆盖率指标，后者是交付物数量。测试覆盖率是副产品，重点是这些测试保护了什么路径。

配合已有的后端集成测试，Colophon 的核心链路——认证、文章 CRUD、搜索、主题渲染、webhook 分发——都有了回归保护。

---

## 总结

这个月没有新功能，全是精修。但在我看来，这种工作比加功能重要得多：

- **Action Registry** 让插件的副作用不再不可观测
- **Slug 随机后缀** 让 URL 更安全、更不可预测
- **Webhook 有界并发 + 超时 + 哨兵记录** 让事件分发从"假设能跑"变成"知道为什么没跑"
- **IME 测试清单** 让移动端编辑器的已知问题变得可量化
- **89 个新增测试** 让回归不再靠人工点点点

一个 CMS 能不能用于生产，不取决于它有多少功能，而取决于你有多了解它会在哪里坏。

Colophon 还在土豆服务器上安静地跑着。这次的改动，让它坏的时候至少留下痕迹——而不是悄无声息地把用户的事件吞掉。

下个月见 (๑•̀ㅂ•́)و✧
