# 使用 i 管理登入外掛

> **Claude 使用 Console API 計費，不使用 Pro / Max 方案額度。**
> 免填 API key 不等於免費，授權前請確認 Console 的組織、工作區與計費權限。

使用者操作統一透過 `i`，不用記輔助程式名稱或手寫連線設定。原生登入外掛
擁有作業系統使用者權限，與工作區 WASM 工具外掛不同，不要混用安裝套件。

## 終端機內完成

啟動 `i`，即使尚未設定模型，也可輸入本機管理命令：

```text
/plugins
```

選擇 ChatGPT 或 Claude，確認權限後安裝，再啟用外掛。輸入 `/login`，
選擇登入外掛，在系統瀏覽器完成授權，然後選擇模型編號或填寫有權限的模型 ID。
成功後自動儲存連線設定，下一條任務即可使用。也可直接輸入：

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

`/cancel` 取消選單；任務執行中不能切換外掛或連線。
授權和模型目錄查詢不發送模型產生請求。只有登入確認和模型選擇都完成後才切換設定；
失敗或取消不覆蓋舊連線，切換前會儲存私有設定備份。

## 命令列管理

### 切換模型和思考等級

終端機中的 `/model` 從當前原生連線外掛讀取模型目錄，可輸入編號選擇；也可直接指定模型。
普通 API-key 連線不探測模型目錄，使用手填模型 ID。不會重新登入或切換帳戶：

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

僅在任務、審核及佇列全部空閒時修改。修改按連線設定儲存，寫入前保留私有備份；
取消目錄選擇不改變設定。僅切換模型時恢復服務商預設思考等級，避免沿用另一模型的參數。
`auto` / `default` 表示不顯式指定等級；其餘可填寫 `none`、`minimal`、`low`、`medium`、
`high`、`xhigh`、`max`，但並非每個模型支持全部等級，目錄也不一定提供能力聲明。
設置不發送付費產生請求，實際可用性由服務商在後續請求時驗證。
Responses 使用 `reasoning.effort`，Chat Completions 使用 `reasoning_effort`；
Claude 使用 `output_config.effort` 與 adaptive thinking，`none` 請求關閉思考，
只適用於支持相應模式的模型。不要將高等級或關閉思考視為所有模型都支持。
參考：[OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning)、
[Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort)。

```sh
i plugins list
i plugins install chatgpt
i plugins enable chatgpt
i login chatgpt
i plugins status chatgpt
i plugins models chatgpt
i plugins logout chatgpt
i plugins disable chatgpt
i plugins update chatgpt
i plugins reconnect chatgpt
i plugins remove chatgpt
```

已有帳戶可用 `i plugins use chatgpt ACCOUNT_ID` 切換；
`i login chatgpt --account ACCOUNT_ID` 重新授權已有注冊，
`i plugins logout chatgpt --account ACCOUNT_ID` 登出指定帳戶。
重新授權只暫存注冊資訊，不複製舊的輪換權杖；模型確認前不會改變舊連線的帳戶。

將 `chatgpt` 換成 `claude` 即可管理 Claude Console 外掛。
安裝和更新需要確認；更新後重新啟用並登入，避免悄悄替換活動連線。
若只更新輔助程式、不切換已有帳戶，可重新啟用後執行 `i plugins reconnect chatgpt`，
保留當前帳戶目錄和模型設置，只將當前連線綁定到驗證過的新程式；不會重新登入或發送產生請求。
安裝時顯示版本查詢、下載位元組數、摘要驗證與儲存進度，可用 `/cancel` 取消。
網路連線超時為 10 秒，每次下載最多 120 秒；失敗不會自動重試或啟用外掛。
GitHub 下載使用終端機的代理伺服器環境變數，不自動繼承瀏覽器代理。需要時可先執行
`HTTPS_PROXY=http://127.0.0.1:3067 i`，地址與端口應替換為自己的代理設定。

也可在 `i` 內透過內置代理管理命令動態切換，預設按使用者儲存至
`~/.i/config/proxy.json`（設置 `I_CONFIG_DIR` 時跟隨該目錄），不寫入工作區：

```text
/proxy                         # 查看預設和各用途的有效規則
/proxy 3067                    # 等同於 http://127.0.0.1:3067
/proxy :3067
/proxy 127.0.0.1:3067
/proxy socks5://127.0.0.1:3067
/proxy no                      # 強制直連，也支持 noproxy
/proxy env                     # 恢復環境變數/模型原設定
/proxy chatgpt 3067            # 僅 ChatGPT 登入、模型目錄及推理
/proxy claude no               # Claude Console 直連
/proxy plugins no              # 外掛下載直連
/proxy model 3067              # 普通模型 API 使用代理
/proxy chatgpt default         # 移除 ChatGPT 覆蓋，跟隨預設
```

規則優先級：用途專屬規則、使用者預設規則、原環境/設定。顯式 `no` 不回退到環境代理。
修改只影響後續操作，不中斷或重放正在執行的請求；瀏覽器及已啟動的獨立服務需另行設定。
支持 HTTP、HTTPS、SOCKS5 和 SOCKS5h；本機授權回呼不走代理。
不支持在代理地址中填寫使用者名、密碼、查詢參數或路徑，避免憑證進入命令歷史。
本命令是宿主內置功能，不向工作區 WASM 外掛開放代理設定或帳戶憑證。
移除安裝選擇會保留舊包與憑證，不代表遠端撤銷。希望登出時，在禁用或移除前
執行 `i plugins logout NAME`；遠端撤銷未確認時，仍需到服務商設置檢查授權。

安裝器從官方 GitHub Release 下載匹配平台的連線附件，驗證 GitHub 提供的
SHA-256 摘要、ZIP 成員允許清單與目標平台，每次呼叫前再檢查程式摘要。
無匹配平台時停止，不執行第三方安裝腳本。摘要驗證完整性，不代替發佈者簽名；
macOS 包目前沒有 Developer-ID 簽名或公證。安裝不修改 PATH，不要求管理員權限。

開發者測試自己構建的連線包，可執行 `i plugins install chatgpt --package /absolute/path/package.zip`，
確認後仍檢查 ZIP 成員與平台，安裝後預設禁用，並在呼叫前驗證程式摘要。
本機包明確標為 `local-build`，不代表經過官方發佈驗證；只安裝自己信任的原生代碼。
瀏覽器顯示回呼已收到，並不代表登入完成；仍須等待終端機完成權杖交換、身份驗證和模型選擇。

舊版產生的使用者級 `desktop.json` 可能仍包含現已進入核心的 `search` / `find_files`。
宿主會在加載時移除可識別的舊內置搜索條目，並將同一使用者設定目錄中的舊內置 worker
綁定到當前程式，不修改磁盤原文件。自定義 worker 和自定義外掛名稱衝突仍保留並回報錯誤，
不會透過跳過重復驗證來繞過注冊規則。此類錯誤發生在模型請求前，無需重新登入。

### 產生失敗的診斷

原生 ChatGPT 連線使用 Responses API，發送 `store:false` 和 `stream:true`，不是 Chat Completions。
終端機優先解析 SSE；相容層也可接收 `application/json` 的完整 Responses 對象，
但必須有 `object:response`、有效 ID、`status:completed` 和通過驗證的輸出。
若響應缺少 `Content-Type`，終端機僅根據本文識別 SSE 或 JSON，再執行相同的完整性驗證；
明確標注為 HTML 等其他類型的響應仍拒絕。失敗診斷只顯示格式分類和位元組數，不顯示本文。
不接受 HTML、Chat Completions 對象、錯誤 JSON 或尚未完成的工具呼叫，不因響應格式變化重發請求。
網頁本機服務正常收到 SSE 時仍即時轉送；缺少類型頭或收到 JSON 時，先使用相同驗證器
驗證完整響應，再向網頁提供有效 SSE。該相容路徑會等待響應完成，不自動重發請求。
網頁顯示的本機 HTTP 400 不一定是上游狀態；安全診斷會區分實際 HTTP、網路或格式失敗。

產生失敗會顯示請求準備、授權、HTTP、響應類型或流解析階段。HTTP 診斷僅保留狀態、
已知錯誤碼/參數名和受限 request ID，不輸出原始錯誤本文、提示詞或權杖。
網路或響應格式失敗不清除現有登入，也不自動切換帳戶、模型或 API-key 計費路徑。

## 設定與憑證

- `<config_dir>/native-plugins.json` 儲存安裝位置、摘要與啟用狀態，不含 OAuth token。
- `<data_dir>/connections/accounts/NAME` 儲存外掛自己的憑證，不交給工作區 WASM 外掛。
- 模型設定由登入選單自動填寫，不需要手工產生 `chatgpt.json`。
- macOS/Linux 使用私有目錄和檔案權限；Windows 繼承使用者目錄 ACL。
- 憑證不作應用層加密，不宣稱存於系統鑰匙串。同使用者惡意原生程式仍可能讀取它們。

不要把上述目錄放進工作區、共享目錄、版本控制或公開備份。
直接填寫 API key 的模型連線仍然可用，無需登入外掛。

## 瀏覽器配對

安裝、啟用後，在執行瀏覽器的同一台電腦執行：

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Claude 將 `chatgpt` 換成 `claude`。保持終端機執行，打開印出的配對鏈接，
在頁面檢查外掛、帳戶和模型後儲存。配對先創建未儲存草稿，不自動替換原連線。
服務重啟後使用新鏈接；鏈接含本機能力金鑰，不要分享。

服務只監聽 `127.0.0.1`，綁定指定頁面 origin，不提供任意 URL 代理。
HTTPS 頁面訪問 loopback 可能需要本機網路權限，本功能不繞過瀏覽器權限或 CORS。
手機不能獨立執行原生外掛。刪除瀏覽器連線不等於原生帳戶登出。

## ChatGPT 邊界與排錯

使用官方 OAuth 動態客戶端注冊、PKCE、獨立宿主標識與憑證儲存，
不讀取個人 `~/.codex/auth.json`，不借用專有客戶端 ID。見
[官方登入文檔](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
與[帳戶和會話](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)。

推理使用公開 Responses API，不是私有 `backend-api`。
資格、工作區政策與授權額度由 OpenAI 決定，登入不保證推理權限。
方案授權固定 `store: false`、`stream: true`；當前預覽忽略最大輸出 token 設置，
見[預覽限制](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)。

失敗會區分授權碼交換、會話刷新和網路錯誤，顯示 HTTP 狀態碼，不顯示權杖端點
原始響應、憑證或授權碼。失敗不自動重放授權或模型請求。授權碼交換被拒絕時，
重新發起登入，並檢查版本、系統時鐘、網路及代理；不能僅憑狀態碼猜測拒絕原因。

## Claude Console 邊界

還需按[官方安裝指南](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart)
安裝 `ant` 並確保它在 PATH 中。連線包不攜帶 `ant`，不會自動執行其安裝腳本。
之後仍透過 `i login claude` 操作。

外掛使用隔離 HOME、`ANTHROPIC_CONFIG_DIR` 與自身 profile，不讀取已有 Claude Code
或個人 ant profile。目前支持一個 Console profile；重新授權可能切換工作區，
後續請求可能發送到新工作區，必要時新建會話。**不是 Pro/Max 訂閱額度。**

## 驗證範圍

自動測試使用隔離目錄及本機 OAuth/TLS fixture，不使用個人真實 token，
不替使用者授權或發送付費請求。離線測試通過不能證明真實帳戶的服務商權限。
輔助程式保留內部 worker/服務協議，使用者操作統一由 `i` 轉發。
