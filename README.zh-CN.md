# Stellar

[English](README.md) | [简体中文](README.zh-CN.md)

> 面向 StarRocks 与 Apache Doris 的 OLAP 集群运维控制台。

<p align="center">
  <a href="https://www.xclaw.live/stellar/"><strong>下载 Stellar 最新版本</strong></a><br>
  <sub>优先获取最新安装包；源码和构建方式见下文。</sub>
</p>

[下载最新版本](https://www.xclaw.live/stellar/) · [查看源码](https://github.com/jlon/starrocks-admin) · [许可证](LICENSE)

<p align="center">
  <img src="docs/images/v2/集群概览.png" alt="Stellar 集群概览" width="100%">
</p>

## 能做什么

| 能力 | 说明 |
| --- | --- |
| 多集群运维 | 在同一控制台管理 StarRocks 与 Doris 集群，查看 FE、BE/CN 节点健康度、资源指标和趋势。 |
| 查询诊断 | 提供实时查询、SQL 工作台、审计日志和 Query Profile 可视化，用于定位执行瓶颈。 |
| 受控智能运维 | 基于集群数据完成取证和事件诊断，提出的操作须经人工确认。 |
| 安全与治理 | 支持组织、用户、角色、资源组、权限申请和操作审计。 |

## 界面预览

<table>
  <tr>
    <td width="50%"><img src="docs/images/v2/智能运维助手.png" alt="智能运维助手"><br><b>智能运维助手</b><br>基于集群证据回答问题，生成可审计的建议与受控操作。</td>
    <td width="50%"><img src="docs/images/v2/profile可视化.png" alt="Query Profile 诊断"><br><b>Query Profile 诊断</b><br>在执行 DAG 中定位瓶颈，并展示根因链路和优化建议。</td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/images/v2/实时查询.png" alt="SQL 工作台"><br><b>SQL 工作台</b><br>浏览 Catalog，执行 SQL，查看结果、图表与执行历史。</td>
    <td width="50%"><img src="docs/images/v2/权限管理.png" alt="权限与审计"><br><b>权限与审计</b><br>按组织和角色管理访问范围，记录关键操作。</td>
  </tr>
</table>

## Docker 快速体验

最新版请从[下载页](https://www.xclaw.live/stellar/)获取；也可用 Docker 快速体验。首次启动会为 `admin` 生成一次性密码，登录后请立即修改。

```bash
docker run -d \
  --name stellar \
  --restart unless-stopped \
  -p 9527:9527 \
  -v "$(pwd)/stellar-data:/data" \
  ghcr.io/jlon/stellar:latest

docker logs stellar 2>&1 | grep 'password:'
```

打开 `http://localhost:9527`，使用 `admin` 和日志中的一次性密码登录。

## 源码

可在 [jlon/starrocks-admin](https://github.com/jlon/starrocks-admin) 浏览源码、提交问题或参与贡献。本地构建：

```bash
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin
make build
```

后端使用 Rust、Axum 和 SQLx；前端使用 Angular、Nebular 和 ECharts。

## 许可证

Stellar 使用 [Apache License 2.0](LICENSE) 发布。

## 捐赠支持

<p align="center">
  <img src="docs/images/wx.png" alt="微信捐赠收款码" width="320"><br>
  <sub>你的捐赠将帮助 Stellar 持续维护和更新。</sub>
</p>
