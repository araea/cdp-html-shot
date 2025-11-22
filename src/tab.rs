use anyhow::{Context, Result};
use serde_json::json;
use std::sync::Arc;

use crate::element::Element;
use crate::general_utils;
use crate::general_utils::next_id;
use crate::transport::Transport;
use crate::transport_actor::TransportResponse;

/// A tab instance.
pub struct Tab {
    pub(crate) transport: Arc<Transport>,
    pub(crate) session_id: String,
    pub(crate) target_id: String,
}

impl Tab {
    /**
    Create a new tab instance.

    # Example
    ```no_run
    use cdp_html_shot::Browser;
    use anyhow::Result;

    #[tokio::main]
    async fn main() -> Result<()> {
        let browser = Browser::new().await?;
        let tab = browser.new_tab().await?;
        Ok(())
    }
    ```
    */
    pub(crate) async fn new(transport: Arc<Transport>) -> Result<Self> {
        let TransportResponse::Response(res) = transport
            .send(json!({
                "id": next_id(),
                "method": "Target.createTarget",
                "params": {
                    "url": "about:blank"
                }
            }))
            .await?
        else {
            panic!()
        };

        let target_id = res
            .result
            .get("targetId")
            .context("Failed to get targetId")?
            .as_str()
            .unwrap();

        let TransportResponse::Response(res) = transport
            .send(json!({
                "id": next_id(),
                "method": "Target.attachToTarget",
                "params": {
                    "targetId": target_id
                }
            }))
            .await?
        else {
            panic!()
        };

        let session_id = res.result["sessionId"].as_str().unwrap();

        Ok(Self {
            transport,
            session_id: String::from(session_id),
            target_id: String::from(target_id),
        })
    }

    /**
    Set the content of the tab and wait for:
    1. Resources to load (Images, Fonts, CSS).
    2. JavaScript to finish executing (DOM stability check).
    3. A minimum "warm-up" period to allow inline scripts to start rendering.
    */
    pub async fn set_content(&self, content: &str) -> Result<&Self> {
        let content_json = serde_json::to_string(content)?;

        let expression = format!(
            r###"
            (async () => {{
                try {{
                    // === 配置项 ===
                    const TOTAL_TIMEOUT = 30000;     // 总超时时间 (30s)
                    const STABILITY_THRESHOLD = 200; // 判定静止的阈值
                    const MIN_RENDER_TIME = 500;     // 最小渲染时间：防止页面加载过快，JS还没开始执行就结束了
                    // ============

                    const startTime = Date.now();

                    await new Promise((resolve, reject) => {{
                        let stabilityTimer = null;
                        let observer = null;
                        let lastMutationTime = Date.now();

                        // 结束函数：清理并返回
                        const finish = () => {{
                            if (observer) observer.disconnect();
                            if (stabilityTimer) clearTimeout(stabilityTimer);
                            resolve(true);
                        }};

                        // 资源加载检查器
                        const checkResources = async () => {{
                            // 超时检查
                            if (Date.now() - startTime > TOTAL_TIMEOUT) {{
                                if (observer) observer.disconnect();
                                reject(new Error('Timeout waiting for page stability'));
                                return;
                            }}

                            // A. 等待基础状态 complete
                            if (document.readyState !== 'complete') {{
                                setTimeout(checkResources, 100);
                                return;
                            }}

                            // B. 等待字体加载
                            await document.fonts.ready;

                            // C. 检查关键资源 (CSS 和 图片)
                            // 注意：document.write 后需要确保能获取到元素
                            const resources = [
                                ...Array.from(document.querySelectorAll('link[rel="stylesheet"]')),
                                ...Array.from(document.images)
                            ];

                            const pending = resources.filter(el => {{
                                if (el.tagName === 'LINK') return !el.sheet;
                                if (el.tagName === 'IMG') return !el.complete;
                                return false;
                            }});

                            if (pending.length > 0) {{
                                setTimeout(checkResources, 100);
                                return;
                            }}

                            // === 资源加载完毕，开始 DOM 稳定性检查 ===
                            startStabilityCheck();
                        }};

                        // DOM 稳定性检查器
                        const startStabilityCheck = () => {{
                            if (observer) return;

                            // 重置最后变动时间为当前，确保观察器启动后的时间窗口有效
                            lastMutationTime = Date.now();

                            observer = new MutationObserver((mutations) => {{
                                lastMutationTime = Date.now();
                            }});

                            // 监听范围扩大到 document.documentElement 以防挂载点在 body 之外
                            observer.observe(document.documentElement, {{
                                childList: true,
                                subtree: true,
                                attributes: true,
                                characterData: true
                            }});

                            // 轮询检查
                            const checkStabilityLoop = () => {{
                                const now = Date.now();
                                const elapsed = now - startTime;        // 总耗时
                                const quietTime = now - lastMutationTime; // 静止时长

                                // 总超时保护
                                if (elapsed > TOTAL_TIMEOUT) {{
                                    finish();
                                    return;
                                }}

                                // 只有同时满足以下两点才算结束：
                                if (quietTime >= STABILITY_THRESHOLD && elapsed >= MIN_RENDER_TIME) {{
                                    // 双重 requestAnimationFrame 确保渲染管道已清空
                                    requestAnimationFrame(() => {{
                                        requestAnimationFrame(() => {{
                                            finish();
                                        }});
                                    }});
                                }} else {{
                                    // 还没静止 或 还没达到最小运行时间，继续等
                                    setTimeout(checkStabilityLoop, 100);
                                }}
                            }};

                            checkStabilityLoop();
                        }};

                        // --- 注入内容开始 ---
                        // 直接使用 JSON 转换后的字符串，不需要 String.raw
                        const htmlContent = {content_json};

                        document.open();
                        document.write(htmlContent);
                        document.close(); // 必须 close，否则 readyState 可能卡住

                        // 启动流程
                        checkResources();
                    }});

                    return 'Page fully stable';
                }} catch (error) {{
                    throw new Error(`Failed to set content: ${{error.message}}`);
                }}
            }})();
            "###,
            content_json = content_json
        );

        let msg_id = next_id();
        let msg = json!({
            "id": msg_id,
            "method": "Runtime.evaluate",
            "params": {
                "expression": expression,
                "awaitPromise": true,
                "returnByValue": true
            }
        })
        .to_string();

        general_utils::send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg)
            .await?;

        Ok(self)
    }

    /**
    Find an element by CSS selector.

    # Example
    ```no_run
    use cdp_html_shot::Browser;
    use anyhow::Result;

    #[tokio::main]
    async fn main() -> Result<()> {
        let browser = Browser::new().await?;
        let tab = browser.new_tab().await?;
        let element = tab.find_element("h1").await?;
        Ok(())
    }
    ```
    */
    pub async fn find_element(&self, selector: &str) -> Result<Element<'_>> {
        let msg_id = next_id();
        let msg = json!({
            "id": msg_id,
            "method": "DOM.getDocument",
            "params": {}
        })
        .to_string();

        let res =
            general_utils::send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg)
                .await?;

        let msg = general_utils::serde_msg(&res);
        let node_id = msg["result"]["root"]["nodeId"].as_u64().unwrap();

        let msg_id = next_id();
        let msg = json!({
            "id": msg_id,
            "method": "DOM.querySelector",
            "params": {
                "nodeId": node_id,
                "selector": selector
            }
        })
        .to_string();

        let res =
            general_utils::send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg)
                .await?;

        let msg = general_utils::serde_msg(&res);

        let node_id = match msg["result"]["nodeId"].as_u64() {
            Some(node_id) => node_id,
            None => return Err(anyhow::anyhow!("Element not found")),
        };

        Element::new(self, node_id).await
    }

    /**
    Close the tab.

    # Example
    ```no_run
    use cdp_html_shot::Browser;
    use anyhow::Result;

    #[tokio::main]
    async fn main() -> Result<()> {
        let browser = Browser::new().await?;
        let tab = browser.new_tab().await?;
        tab.close().await?;
        Ok(())
    }
    ```
    */
    pub async fn activate(&self) -> Result<&Self> {
        let msg_id = next_id();
        let msg = json!({
            "id": msg_id,
            "method": "Target.activateTarget",
            "params": {
                "targetId": self.target_id
            }
        })
        .to_string();

        general_utils::send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg)
            .await?;

        Ok(self)
    }

    /**
    Navigate to a URL.

    # Warning

    This API does not wait for the page to load, it is only used to navigate to local HTML files,
    which is convenient for getting font and other resources.

    # Example
    ```no_run
    use cdp_html_shot::Browser;
    use anyhow::Result;
    use tokio::time;

    #[tokio::main]
    async fn main() -> Result<()> {
        let browser = Browser::new().await?;
        let tab = browser.new_tab().await?;
        tab.goto("https://www.rust-lang.org/").await?;
        time::sleep(time::Duration::from_secs(5)).await;
        Ok(())
    }
    ```
    */
    pub async fn goto(&self, url: &str) -> Result<&Self> {
        let msg_id = next_id();
        let msg = json!({
            "id": msg_id,
            "method": "Page.navigate",
            "params": {
                "url": url
            }
        })
        .to_string();

        general_utils::send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg)
            .await?;

        Ok(self)
    }

    /**
    Close the tab.

    # Example
    ```no_run
    use cdp_html_shot::Browser;
    use anyhow::Result;

    #[tokio::main]
    async fn main() -> Result<()> {
        let browser = Browser::new().await?;
        let tab = browser.new_tab().await?;
        tab.close().await?;
        Ok(())
    }
    ```
    */
    pub async fn close(&self) -> Result<()> {
        let msg_id = next_id();
        let msg = json!({
            "id": msg_id,
            "method": "Target.closeTarget",
            "params": {
                "targetId": self.target_id
            }
        })
        .to_string();

        general_utils::send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg)
            .await?;

        Ok(())
    }
}
