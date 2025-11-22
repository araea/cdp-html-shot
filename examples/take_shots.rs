use anyhow::Result;
use base64::Engine;
use cdp_html_shot::{Browser, Tab};
use futures::future::try_join_all;
use std::fs;
use tokio::try_join;

async fn take_screenshot(tab: Tab, filename: &str) -> Result<()> {
    tab.set_content(HTML).await?;
    let element = tab.find_element("#title_and_result").await?;
    let base64 = element.screenshot().await?;
    tab.close().await?;
    let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;

    let dir = std::env::current_dir()?.join("cache");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(filename), img_data)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // let browser = Browser::new().await?;
    let browser = Browser::new_with_head().await?;

    let (tab1, tab2, tab3, tab4, tab5, tab6) = try_join!(
        browser.new_tab(),
        browser.new_tab(),
        browser.new_tab(),
        browser.new_tab(),
        browser.new_tab(),
        browser.new_tab(),
    )?;

    let screenshot_tasks = vec![
        take_screenshot(tab1, "test1.jpeg"),
        take_screenshot(tab2, "test2.jpeg"),
        take_screenshot(tab3, "test3.jpeg"),
        take_screenshot(tab4, "test4.jpeg"),
        take_screenshot(tab5, "test5.jpeg"),
        take_screenshot(tab6, "test6.jpeg"),
    ];

    try_join_all(screenshot_tasks).await?;
    Ok(())
}

const HTML: &str = r#"<!DOCTYPE html>
<html lang="zh" style="height:100%">
<head>
    <base href="https://shindanmaker.com/">
    <link rel="stylesheet" href="/css/app.css">
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1.0,minimum-scale=1.0">
    <script src="/js/shindan.js" defer></script>
    <!-- SCRIPTS -->
    <title>ShindanMaker</title>
</head>
<body class="" style="position:relative;min-height:100%;top:0">
    <div id="main-container">
        <div id="main"><div id="title_and_result"> <div class="shindanTitleImageContainer"> <a href="https://shindanmaker.com/a/1252750" class="text-white text-decoration-none"><img loading="eager" decoding="async" class="img-fluid" height="504" fetchpriority="high" alt="人设生成器，但是H文♡" src="https://pic.shindanmaker.com/shindantitle/1252750/img/784f639e44c8ae033efefb5ba4e3303d37f6da11_head.jpg?v=791879030eaf5c4f5107ae6248eb450d86cdf4e0-a" width="960"></a> </div> <div class="mx-0" id="shindanResultBlock" name="shindanResultBlock" data-shindan_title_image="https://pic.shindanmaker.com/shindantitle/1252750/img/784f639e44c8ae033efefb5ba4e3303d37f6da11_head.jpg?v=791879030eaf5c4f5107ae6248eb450d86cdf4e0-a"> <span id="shindanResultTitle" class="d-block text-center text-nowrap overflow-hidden font-weight-bold px-2 mb-0"> <span class="d-block py-3 py-sm-4" id="shindanResultTitleText"> 診断結果 </span> </span> <span class="d-block" id="shindanResultContainer"> <span id="shindanResultHeight"> <span id="shindanResultCell"> <span id="shindanResultContent" class="d-block py-4 px-3 px-sm-4 text-break text-center "> <span id="shindanResult" class="d-inline-block text-left" data-context="{&quot;values&quot;:{&quot;name&quot;:&quot;test_user&quot;,&quot;like&quot;:&quot;\u8986\u9762\u7cfb&quot;,&quot;type&quot;:&quot;\u5e74\u4e0a&quot;,&quot;mingan&quot;:&quot;\u8033\u6735(\u5439\u6c14\u4f1a\u98a4\u6296\uff0c\u8214\u8210\u4f1a\u817f\u8f6f)&quot;,&quot;rouren&quot;:&quot;\u6bd4\u8f83\u597d&quot;,&quot;weidao&quot;:&quot;\u94c3\u5170&quot;,&quot;xingyu&quot;:&quot;\u666e\u901a\u4eba\u5e73\u5747\u6c34\u5e73&quot;,&quot;shencai&quot;:&quot;\u666e\u901a\u4eba&quot;,&quot;kaifang&quot;:&quot;\u63a5\u53d7\u7edd\u5927\u90e8\u5206\u5947\u5947\u602a\u602a\u7684\u73a9\u6cd5&quot;,&quot;jishu&quot;:&quot;\u8001\u624b&quot;,&quot;naijiu&quot;:&quot;\u6bd4\u8f83\u6301\u4e45&quot;,&quot;yanse&quot;:&quot;\u5ae9\u7eff&quot;,&quot;yanse_1&quot;:&quot;\u53e4\u94dc\u8272&quot;,&quot;tongse&quot;:&quot;\u53e4\u94dc\u8272&quot;,&quot;fuse&quot;:&quot;\u74f7\u767d&quot;,&quot;tezheng&quot;:&quot;\u773c\u955c&quot;,&quot;shendu&quot;:&quot;9CM(\u2022\u0301\u2304\u2022\u0301\u0e51)\u0aed\u5c0f\u5c0f\u7684\u4e5f\u5f88\u53ef\u7231&quot;,&quot;shu&quot;:8,&quot;shuzi&quot;:53,&quot;banlv&quot;:&quot;\u5426&quot;,&quot;xing&quot;:&quot;\u6b3a\u8d1f\u4eba\u7684\u9014\u5f84&quot;,&quot;yanse_0&quot;:&quot;\u54c1\u7ea2&quot;,&quot;tongse_0&quot;:&quot;\u53e4\u94dc\u8272&quot;,&quot;fuse_0&quot;:&quot;\u5c0f\u9ea6\u8272&quot;,&quot;tezheng_0&quot;:&quot;\u732b\u820c&quot;,&quot;like_0&quot;:&quot;\u63d2\u5165\u5f0f\u5c3e\u5df4&quot;,&quot;like_1&quot;:&quot;\u8eab\u4f53\u6539\u9020&quot;,&quot;like_2&quot;:&quot;\u80f6\u8863&quot;,&quot;type_0&quot;:&quot;switch\uff08\u53cc\u5c5e\u6027\uff09&quot;,&quot;mingan_0&quot;:&quot;\u773c\u89d2&quot;,&quot;mingan_1&quot;:&quot;\u8033\u5782&quot;,&quot;rouren_0&quot;:&quot;\u4e00\u822c&quot;,&quot;weidao_0&quot;:&quot;\u6843\u5b50&quot;,&quot;xingyu_0&quot;:&quot;\u6bd4\u8f83\u5f3a&quot;,&quot;shencai_0&quot;:&quot;\u7626\u5f31&quot;,&quot;kaifang_0&quot;:&quot;\u63a5\u53d7\u7edd\u5927\u90e8\u5206\u5947\u5947\u602a\u602a\u7684\u73a9\u6cd5&quot;,&quot;jishu_0&quot;:&quot;\u7b28\u62d9&quot;,&quot;naijiu_0&quot;:&quot;\u6bd4\u8f83\u6301\u4e45&quot;,&quot;shendu_0&quot;:&quot;22CM\uff08\u30fb\u25a1\u30fb\uff1b\uff09\u597d\u957f!&quot;,&quot;banlv_0&quot;:&quot;\u5426&quot;,&quot;xing_0&quot;:&quot;\u7231\u60c5&quot;},&quot;format_tags&quot;:[],&quot;template&quot;:&quot;\u300e\u60a8\u5728H\u6587\u4e2d\u7684\u4eba\u8bbe\u662f\uff1a\u300f\n\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\n\u540d\u5b57\uff1a${name}\n\u53d1\u8272\uff1a${yanse}\n\u77b3\u8272\uff1a${tongse}\n\u80a4\u8272\uff1a${fuse}\n\u7279\u5f81\uff1a${tezheng}\nXP\uff1a${like!}\u3001${like_1!}\n\u4e0d\u559c\u6b22\uff1a${like_2!}\n\u5c5e\u6027\uff1a${type}\n\u654f\u611f\u5e26\uff1a${mingan!}\u3001${mingan_1!}\n\u67d4\u97e7\u5ea6\uff1a${rouren}\n\u5473\u9053\uff1a${weidao}\n\u6027\u6b32\uff1a${xingyu}\n\u8eab\u6750\uff1a${shencai}\n\u5f00\u653e\u7a0b\u5ea6\uff1a${kaifang}\n\u6280\u672f\uff1a${jishu}\n\u8010\u4e45\uff1a${naijiu}\n\u957f\u5ea6\/\u6df1\u5ea6\uff1a${shendu}\n\u505a\u8fc7\u2661\u4eba\u6570\uff1a${shu}\u4eba\n\u505a\u2661\u6b21\u6570\uff1a${shuzi}\u6b21\n\u76ee\u524d\u662f\u5426\u6709\u56fa\u5b9a\u6027\u4f34\u4fa3\uff1a${banlv}\n\u6027\u5bf9TA\u800c\u8a00\u662f\uff1a${xing}\n\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\n\u300e\u5e0c\u671b\u4f60\u4f1a\u559c\u6b22\u81ea\u5df1\u7684R18\u4eba\u8bbe~\uff08\u2661\ud81a\udd66\u2661 \uff09\u300f&quot;}" data-blocks="[{&quot;type&quot;:&quot;text&quot;,&quot;content&quot;:&quot;\u300e\u60a8\u5728H\u6587\u4e2d\u7684\u4eba\u8bbe\u662f\uff1a\u300f\n\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\n\u540d\u5b57\uff1a&quot;},{&quot;type&quot;:&quot;user_input&quot;,&quot;variable&quot;:&quot;&quot;,&quot;value&quot;:&quot;test_user&quot;,&quot;styles&quot;:{&quot;bold&quot;:true}},{&quot;type&quot;:&quot;text&quot;,&quot;content&quot;:&quot;\n\u53d1\u8272\uff1a\u54c1\u7ea2\n\u77b3\u8272\uff1a\u53e4\u94dc\u8272\n\u80a4\u8272\uff1a\u5c0f\u9ea6\u8272\n\u7279\u5f81\uff1a\u732b\u820c\nXP\uff1a\u63d2\u5165\u5f0f\u5c3e\u5df4\u3001\u8eab\u4f53\u6539\u9020\n\u4e0d\u559c\u6b22\uff1a\u80f6\u8863\n\u5c5e\u6027\uff1aswitch\uff08\u53cc\u5c5e\u6027\uff09\n\u654f\u611f\u5e26\uff1a\u773c\u89d2\u3001\u8033\u5782\n\u67d4\u97e7\u5ea6\uff1a\u4e00\u822c\n\u5473\u9053\uff1a\u6843\u5b50\n\u6027\u6b32\uff1a\u6bd4\u8f83\u5f3a\n\u8eab\u6750\uff1a\u7626\u5f31\n\u5f00\u653e\u7a0b\u5ea6\uff1a\u63a5\u53d7\u7edd\u5927\u90e8\u5206\u5947\u5947\u602a\u602a\u7684\u73a9\u6cd5\n\u6280\u672f\uff1a\u7b28\u62d9\n\u8010\u4e45\uff1a\u6bd4\u8f83\u6301\u4e45\n\u957f\u5ea6\/\u6df1\u5ea6\uff1a22CM\uff08\u30fb\u25a1\u30fb\uff1b\uff09\u597d\u957f!\n\u505a\u8fc7\u2661\u4eba\u6570\uff1a8\u4eba\n\u505a\u2661\u6b21\u6570\uff1a53\u6b21\n\u76ee\u524d\u662f\u5426\u6709\u56fa\u5b9a\u6027\u4f34\u4fa3\uff1a\u5426\n\u6027\u5bf9TA\u800c\u8a00\u662f\uff1a\u7231\u60c5\n\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\u2014\n\u300e\u5e0c\u671b\u4f60\u4f1a\u559c\u6b22\u81ea\u5df1\u7684R18\u4eba\u8bbe~\uff08\u2661\ud81a\udd66\u2661 \uff09\u300f&quot;}]"></span> </span> </span> </span> </span> </div></div></div>
    </div>
</body>
</html>"#;
