import "../components/ControlPanel/ControlPanel.css";

export function HelpPage() {
  return (
    <div className="page-layout page-layout--single">
      <section className="control-panel page-section help-page">
        <h2 className="page-section__title">使用說明</h2>

        <div className="help-block">
          <h3>基本流程</h3>
          <ol>
            <li>選擇章節存放目錄，會自動在該目錄下建立一個子資料夾，資料夾名稱為小說名稱。</li>
            <li>在填入小說目錄 URL。</li>
            <li>按「① 開啟抓取視窗」，在該視窗手動完成 Cloudflare 驗證，直到下載完成，不要關閉視窗</li>
            <li>確認目錄頁正常顯示後，按「② 列出章節」。</li>
            <li>在右側章節列表勾選要下載的章節，按「③ 下載已選」。</li>
          </ol>
        </div>

        <div className="help-block">
          <h3>章節列表標記</h3>
          <ul>
            <li>🔒 — 需登入，無法下載</li>
            <li>✓ — 本地已存在該章節</li>
          </ul>
        </div>

        <div className="help-block">
          <h3>其他功能</h3>
          <ul>
            <li>
              <strong>強制停止</strong> — 下載進行中可中斷任務。
            </li>
            <li>
              <strong>Debug → 探測目前抓取頁</strong> — 將頁面結構分析報告寫入檔案，適用於無法開 F12 的情況。
            </li>
          </ul>
        </div>

        <div className="help-block">
          <h3>設定項目</h3>
          <ul>
            <li>
              <strong>強制覆蓋已存在章節</strong> — 重新下載已有檔案的章節。
            </li>
            <li>
              <strong>章節間隔</strong> — 控制每章之間的請求間隔（毫秒）。
            </li>
          </ul>
        </div>

        <p className="help-attribution">
          Animated icons by{" "}
          <a href="https://lordicon.com/" target="_blank" rel="noreferrer">
            Lordicon.com
          </a>
        </p>
      </section>
    </div>
  );
}
