export const languages=['zh-CN','zh-TW','en','ja','ko','fr','de'];
// Each row follows the language order above; missing translations fail at build/test time.
const messages={
    'nav.docs':['文档','文件','Docs','ドキュメント','문서','Documentation','Dokumentation'],
    'nav.plugins':['插件开发','外掛開發','Build plugins','プラグイン開発','플러그인 개발','Créer un plugin','Plugins entwickeln'],
    'nav.open':['打开工作台','開啟工作台','Open workspace','ワークスペースを開く','작업 공간 열기','Ouvrir l’espace','Workspace öffnen'],
    'nav.home':['lninl 首页','lninl 首頁','lninl home','lninl ホーム','lninl 홈','Accueil lninl','lninl Startseite'],
    'nav.main':['主导航','主導覽','Main navigation','メインナビゲーション','주 탐색','Navigation principale','Hauptnavigation'],
    'nav.language':['界面语言','介面語言','Interface language','表示言語','표시 언어','Langue de l’interface','Oberflächensprache'],
    'nav.theme':['切换明暗主题','切換明暗主題','Toggle light or dark theme','明暗テーマを切り替え','밝은 테마와 어두운 테마 전환','Changer de thème clair ou sombre','Helles oder dunkles Design wechseln'],
    'nav.skip':['跳到正文','跳至正文','Skip to content','本文へ移動','본문으로 이동','Aller au contenu','Zum Inhalt springen'],
    'search.label':['搜索文档','搜尋文件','Search docs','ドキュメント検索','문서 검색','Rechercher','Dokumentation suchen'],
    'search.placeholder':['插件、OPFS、模型连接…','外掛、OPFS、模型連線…','Plugins, OPFS, model connections…','プラグイン、OPFS、モデル接続…','플러그인, OPFS, 모델 연결…','Plugins, OPFS, connexion aux modèles…','Plugins, OPFS, Modellverbindungen…'],
    'search.results':['搜索结果','搜尋結果','Search results','検索結果','검색 결과','Résultats de recherche','Suchergebnisse'],
    'search.empty':['未找到结果，请尝试其他关键词。','找不到結果，請嘗試其他關鍵字。','No results. Try another keyword.','見つかりません。別のキーワードをお試しください。','결과가 없습니다. 다른 검색어를 입력하세요.','Aucun résultat. Essayez un autre mot-clé.','Keine Ergebnisse. Versuche ein anderes Suchwort.'],
    'search.error':['搜索暂不可用，可从文档目录浏览。','搜尋暫時無法使用，可從文件目錄瀏覽。','Search is unavailable. Browse the documentation instead.','検索を利用できません。ドキュメント一覧をご覧ください。','검색을 사용할 수 없습니다. 문서 목록을 이용하세요.','Recherche indisponible. Consultez la documentation.','Suche nicht verfügbar. Nutze das Dokumentationsverzeichnis.'],
    'footer.nav':['页脚导航','頁尾導覽','Footer navigation','フッターナビゲーション','바닥글 탐색','Navigation de pied de page','Fußzeilennavigation'],
    'footer.contribute':['参与贡献','參與貢獻','Contribute','コントリビューション','기여하기','Contribuer','Mitwirken'],
    'footer.security':['安全','安全','Security','セキュリティ','보안','Sécurité','Sicherheit'],
    'footer.licenses':['第三方许可','第三方授權','Third-party licenses','サードパーティーライセンス','타사 라이선스','Licences tierces','Drittlizenzen'],
    'footer.source':['源码','原始碼','Source code','ソースコード','소스 코드','Code source','Quellcode'],
    'footer.privacy':['不使用分析追踪。模型请求由你选择的连接处理；请勿在公开反馈中提交密钥。','不使用分析追蹤。模型請求由你選擇的連線處理；請勿在公開回饋中提交金鑰。','No analytics tracking. Your selected connection handles model requests. Never include API keys in public reports.','解析トラッキングは使用しません。モデルへのリクエストは選択した接続先が処理します。公開の報告に API キーを含めないでください。','분석 추적을 사용하지 않습니다. 모델 요청은 선택한 연결에서 처리합니다. 공개 피드백에 API 키를 포함하지 마세요.','Aucun suivi analytique. La connexion choisie traite les requêtes au modèle. Ne publiez jamais vos clés API.','Kein Analytics-Tracking. Deine gewählte Verbindung verarbeitet Modellanfragen. Veröffentliche niemals API-Schlüssel.'],
    'hero.first':['小核心。','小核心。','Small core.','小さなコア。','작은 코어.','Petit noyau.','Kleiner Kern.'],
    'hero.second':['大作为。','大作為。','Big moves.','大きな可能性。','큰 가능성.','Grandes idées.','Viel bewegen.'],
    'hero.description':['你的模型，你的工具，你的工作区。一个用 Rust 构建、在终端与浏览器运行的可扩展编码 Agent。','你的模型、你的工具、你的工作區。以 Rust 打造，在終端機與瀏覽器執行的可擴充程式開發 Agent。','Your model. Your tools. Your workspace. An extensible coding agent built in Rust, running in your terminal and browser.','モデルも、ツールも、ワークスペースも自分で選ぶ。Rust 製の拡張可能なコーディング Agent を、ターミナルとブラウザーで。','내 모델, 내 도구, 내 작업 공간. 터미널과 브라우저에서 실행되는 Rust 기반의 확장 가능한 코딩 에이전트입니다.','Votre modèle, vos outils, votre espace. Un agent de programmation extensible en Rust, dans le terminal et le navigateur.','Dein Modell. Deine Tools. Dein Workspace. Ein erweiterbarer Coding-Agent in Rust, für Terminal und Browser.'],
    'hero.open':['打开浏览器工作台','開啟瀏覽器工作台','Launch browser workspace','ブラウザーで始める','브라우저 작업 공간 열기','Lancer l’espace web','Browser-Workspace starten'],
    'hero.source':['探索源码','探索原始碼','Explore the source','ソースコードを見る','소스 살펴보기','Explorer le code','Quellcode entdecken'],
    'hero.note':['无需本站账号。连接你自己的模型 API。','不需本站帳號。連接你自己的模型 API。','No account here. Bring your own model API.','本サイトのアカウントは不要。自分のモデル API を接続。','사이트 계정 없이 내 모델 API를 연결하세요.','Pas de compte requis. Connectez votre propre API de modèle.','Kein Konto nötig. Verbinde deine eigene Modell-API.'],
    'demo.choose':['选择演示任务','選擇示範任務','Choose a demo task','デモのタスクを選択','데모 작업 선택','Choisir une démonstration','Demo-Aufgabe wählen'],
    'demo.website':['构建页面','建立頁面','Build a page','ページ作成','페이지 만들기','Créer une page','Seite erstellen'],
    'demo.refactor':['修改代码','修改程式碼','Refactor code','コード修正','코드 수정','Modifier le code','Code ändern'],
    'demo.plugin':['扩展工具','擴充工具','Add a tool','ツール拡張','도구 확장','Ajouter un outil','Tool erweitern'],
    'demo.prompt.website':['创建一个响应式页面，打包下载。','建立響應式頁面，打包下載。','Build a responsive page and export a ZIP.','レスポンシブなページを作り、ZIP にまとめて。','반응형 페이지를 만들고 ZIP으로 내보내세요.','Crée une page responsive et exporte un ZIP.','Erstelle eine responsive Seite und exportiere ein ZIP.'],
    'demo.prompt.refactor':['找到相关代码，一次应用必要修改。','找出相關程式碼，一次套用必要修改。','Find the relevant code. Apply focused edits.','関連コードを探して、必要な変更をまとめて。','관련 코드를 찾아 필요한 부분만 수정하세요.','Trouve le code concerné et applique des modifications ciblées.','Finde den relevanten Code und ändere gezielt.'],
    'demo.prompt.plugin':['用已安装的插件统计文件字数。','以已安裝的外掛統計檔案字數。','Count words with an installed plugin.','インストール済みプラグインで単語数を集計して。','설치된 플러그인으로 단어 수를 세어 주세요.','Compte les mots avec un plugin installé.','Zähle Wörter mit einem installierten Plugin.'],
    'demo.result.website':['相关文件已组织为 ZIP','相關檔案已整理為 ZIP','Project files organized as a ZIP','ファイルを ZIP に整理','프로젝트 파일을 ZIP으로 정리','Fichiers regroupés en ZIP','Projektdateien als ZIP gebündelt'],
    'demo.result.refactor':['只修改需要修改的地方','只修改需要修改之處','Only the necessary code changes','必要な箇所だけを変更','필요한 코드만 수정','Seul le code nécessaire est modifié','Nur notwendige Änderungen'],
    'demo.result.plugin':['有界输入，结构化结果','有界輸入，結構化結果','Bounded input. Structured output.','上限付き入力、構造化された結果','제한된 입력, 구조화된 출력','Entrée bornée, résultat structuré','Begrenzte Eingabe, strukturierte Ausgabe'],
    'demo.local':['设备上的工作区','裝置上的工作區','Workspace on your device','デバイス上のワークスペース','내 기기의 작업 공간','Espace sur votre appareil','Workspace auf deinem Gerät'],
    'demo.disclaimer':['交互示例 · 不发送模型请求','互動示範 · 不傳送模型請求','Interactive demo · no model requests','操作デモ · モデルへのリクエストなし','대화형 데모 · 모델 요청 없음','Démo interactive · aucun appel au modèle','Interaktive Demo · keine Modellanfragen'],
    'spec.core':['共享核心','共用核心','Shared core','共通コア','공유 코어','Noyau partagé','Gemeinsamer Kern'],
    'spec.browser':['浏览器原生运行','瀏覽器原生執行','Runs in your browser','ブラウザーで実行','브라우저에서 실행','Dans votre navigateur','Läuft im Browser'],
    'spec.files':['设备本地文件','裝置本機檔案','On-device files','デバイス上のファイル','기기에 저장된 파일','Fichiers sur l’appareil','Dateien auf deinem Gerät'],
    'spec.open':['开源，可扩展','開源，可擴充','Open and extensible','オープンで拡張可能','오픈 소스, 확장 가능','Libre et extensible','Offen und erweiterbar'],
    'runtime.title':['一个核心。两种工作方式。','一個核心。兩種工作方式。','One core. Two ways to work.','ひとつのコア。ふたつの使い方。','하나의 코어. 두 가지 작업 방식.','Un noyau. Deux façons de travailler.','Ein Kern. Zwei Arbeitsweisen.'],
    'runtime.description':['从一行命令到一个浏览器标签页，工具与权限保持清晰。','從一行指令到一個瀏覽器分頁，工具與權限始終清晰。','From a command line to a browser tab. Clear tools and explicit permissions.','コマンドラインでもブラウザーでも。明確なツールと権限。','명령줄에서 브라우저 탭까지. 도구와 권한은 명확하게.','De la ligne de commande à l’onglet web, des outils clairs et des permissions explicites.','Von der Kommandozeile bis zum Browser-Tab. Klare Tools und eindeutige Berechtigungen.'],
    'native.title':['靠近你的开发环境。','貼近你的開發環境。','Right where you develop.','開発環境のすぐそばで。','개발 환경 가까이에서.','Au cœur de votre environnement.','Direkt in deiner Umgebung.'],
    'native.description':['Rust CLI、常驻 NDJSON worker、可替换的模型 provider。用标准输入输出连接自己的工具。','Rust CLI、常駐 NDJSON worker、可替換的模型 provider。透過標準輸入輸出連接自己的工具。','A Rust CLI, persistent NDJSON workers, and replaceable model providers. Connect your tools over standard I/O.','Rust CLI、常駐 NDJSON worker、交換可能なモデル provider。標準入出力で自分のツールを接続。','Rust CLI, 상주 NDJSON 워커, 교체 가능한 모델 제공자. 표준 입출력으로 내 도구를 연결하세요.','CLI Rust, workers NDJSON persistants et fournisseurs de modèles interchangeables. Branchez vos outils par les entrées-sorties standard.','Rust-CLI, persistente NDJSON-Worker und austauschbare Modellanbieter. Verbinde deine Tools über Standard-I/O.'],
    'native.link':['安装与启动','安裝與啟動','Install and run','インストールと起動','설치 및 실행','Installer et lancer','Installieren und starten'],
    'browser.title':['一个标签页，就是工作区。','一個分頁，就是工作區。','Your workspace is a tab away.','タブひとつが、ワークスペースに。','탭 하나가 작업 공간으로.','Un onglet, tout un espace.','Ein Tab wird zum Workspace.'],
    'browser.description':['持久 OPFS 文件、多会话、流式输出、HTML 预览和 ZIP 下载。无需本地开发服务器，也可安装为 PWA。','持久 OPFS 檔案、多對話、串流輸出、HTML 預覽與 ZIP 下載。不需本機開發伺服器，也可安裝為 PWA。','Persistent OPFS files, multiple conversations, streaming, HTML previews, and ZIP exports. No local dev server needed. Installable as a PWA.','永続 OPFS、多会話、ストリーミング、HTML プレビュー、ZIP 出力。ローカルサーバー不要。PWA としてインストール可能。','영구 OPFS 파일, 여러 대화, 스트리밍, HTML 미리보기, ZIP 내보내기. 로컬 개발 서버 없이 PWA로도 설치할 수 있습니다.','Fichiers OPFS persistants, conversations multiples, streaming, aperçu HTML et export ZIP. Sans serveur local, installable en PWA.','Persistente OPFS-Dateien, mehrere Gespräche, Streaming, HTML-Vorschau und ZIP-Export. Ohne lokalen Dev-Server. Als PWA installierbar.'],
    'browser.link':['直接开始','直接開始','Start building','今すぐ始める','바로 시작','Commencer','Direkt loslegen'],
    'workflow.title':['让工具做检索，让模型做判断。','讓工具檢索，讓模型判斷。','Tools retrieve. Models decide.','検索はツールに。判断はモデルに。','검색은 도구가, 판단은 모델이.','Les outils cherchent. Le modèle décide.','Tools suchen. Modelle entscheiden.'],
    'workflow.description':['先定位、再读取，一次处理相关修改。减少无关上下文和往返，不靠反复猜路径。','先定位、再讀取，一次處理相關修改。減少無關上下文與往返，不必反覆猜測路徑。','Locate first, read what matters, then batch related edits. Less irrelevant context and fewer trips spent guessing paths.','まず特定し、必要な範囲を読み、関連する変更をまとめる。不要な文脈とパスの推測を減らす。','먼저 위치를 찾고, 필요한 부분을 읽고, 관련 수정을 묶어 처리합니다. 불필요한 문맥과 경로 추측을 줄입니다.','Localiser, lire l’essentiel, puis regrouper les modifications. Moins de contexte inutile et d’allers-retours pour deviner les chemins.','Erst finden, gezielt lesen, dann zusammengehörige Änderungen bündeln. Weniger irrelevanter Kontext und weniger Raten bei Pfaden.'],
    'workflow.find':['定位','定位','Locate','特定','찾기','Localiser','Finden'],
    'workflow.find.detail':['递归路径与带定位的搜索。','遞迴路徑與附帶位置的搜尋。','Recursive paths and precise search.','再帰パスと位置付き検索。','재귀 경로 및 위치 기반 검색.','Chemins récursifs et recherche précise.','Rekursive Pfade und präzise Suche.'],
    'workflow.read':['读取','讀取','Read','読み込み','읽기','Lire','Lesen'],
    'workflow.read.detail':['批量读取真正需要的范围。','批次讀取真正需要的範圍。','Batch only the ranges you need.','必要な範囲だけを一括取得。','필요한 범위만 일괄 읽기.','Lire uniquement les plages utiles.','Nur benötigte Bereiche gebündelt lesen.'],
    'workflow.edit':['修改','修改','Edit','編集','수정','Modifier','Ändern'],
    'workflow.edit.detail':['校验并应用一组文件修改。','驗證並套用一組檔案修改。','Validate and apply grouped edits.','一連の変更を検証して適用。','묶음 수정을 검증하고 적용.','Valider et appliquer les modifications.','Änderungen prüfen und anwenden.'],
    'workflow.deliver':['交付','交付','Deliver','出力','내보내기','Livrer','Liefern'],
    'workflow.deliver.detail':['把结果打包成 ZIP。','將結果打包為 ZIP。','Package the result as a ZIP.','結果を ZIP にまとめる。','결과를 ZIP으로 묶기.','Regrouper le résultat en ZIP.','Ergebnis als ZIP verpacken.'],
    'workflow.note':['实际调用轮次与 token 消耗取决于任务和模型。','實際呼叫次數與 token 用量取決於任務和模型。','Actual calls and token use depend on the task and model.','呼び出し回数と token 使用量はタスクとモデルに依存します。','실제 호출 횟수와 토큰 사용량은 작업과 모델에 따라 달라집니다.','Le nombre d’appels et de tokens dépend de la tâche et du modèle.','Aufrufe und Tokenverbrauch hängen von Aufgabe und Modell ab.'],
    'workflow.link':['查看文件工具契约 →','查看檔案工具契約 →','File tool contracts →','ファイルツールの仕様 →','파일 도구 명세 →','Contrats des outils fichiers →','Datei-Tool-Spezifikation →'],
    'plugin.title':['你的能力，不必塞进核心。','你的能力，不必塞進核心。','Extend the tools. Not the core.','コアではなく、ツールを拡張。','코어 대신 도구를 확장하세요.','Étendez les outils, pas le noyau.','Tools erweitern. Nicht den Kern.'],
    'plugin.description':['用 Rust 构建 WASM 插件，或连接原生 worker。从源码到安装，用明确的 manifest、ABI 和权限边界扩展 Agent。','以 Rust 建立 WASM 外掛，或連接原生 worker。從原始碼到安裝，以明確的 manifest、ABI 與權限邊界擴充 Agent。','Build WASM plugins in Rust or connect native workers. Go from source to installation with explicit manifests, ABIs, and permission boundaries.','Rust で WASM プラグインを構築、またはネイティブ worker を接続。manifest、ABI、権限の境界を明示して拡張。','Rust로 WASM 플러그인을 만들거나 네이티브 워커를 연결하세요. 명확한 manifest, ABI, 권한 경계로 확장합니다.','Créez des plugins WASM en Rust ou connectez des workers natifs. Du code à l’installation, avec manifestes, ABI et permissions explicites.','Baue WASM-Plugins in Rust oder verbinde native Worker. Vom Quellcode zur Installation mit klaren Manifesten, ABIs und Berechtigungsgrenzen.'],
    'plugin.start':['编写第一个插件 →','撰寫第一個外掛 →','Build your first plugin →','最初のプラグインを作る →','첫 플러그인 만들기 →','Créer votre premier plugin →','Erstes Plugin entwickeln →'],
    'plugin.wasm':['WASM API 参考 ↗','WASM API 參考 ↗','WASM API reference ↗','WASM API リファレンス ↗','WASM API 참조 ↗','Référence API WASM ↗','WASM-API-Referenz ↗'],
    'plugin.native':['原生 worker 参考 ↗','原生 worker 參考 ↗','Native worker reference ↗','ネイティブ worker リファレンス ↗','네이티브 워커 참조 ↗','Référence worker natif ↗','Native Worker-Referenz ↗'],
    'boundary.title':['连接由你选，边界说清楚。','連線由你選，邊界說清楚。','Your connections. Clear boundaries.','接続は自分で選ぶ。境界は明確に。','연결은 직접 선택하고, 경계는 명확하게.','Vos connexions. Des limites claires.','Deine Verbindungen. Klare Grenzen.'],
    'boundary.keys':['自带模型连接','自備模型連線','Bring your own model','自分のモデルを接続','내 모델 연결','Votre propre modèle','Dein eigenes Modell'],
    'boundary.keys.detail':['直连请求只发给你选择的 API；自建 Relay 会经由你的中继。密钥可选择仅在当前浏览器保存。','直連請求只傳送至你選擇的 API；自建 Relay 會經由你的中繼。金鑰可選擇僅儲存在目前瀏覽器。','Direct requests go to your chosen API; your own Relay passes them through your server. Optionally remember keys in this browser.','直結は選択した API へ、自前 Relay は自分のサーバー経由へ。キーは任意でこのブラウザーに保存できます。','직접 요청은 선택한 API로, 자체 Relay 요청은 내 서버를 거칩니다. 키는 선택적으로 현재 브라우저에 저장합니다.','En direct, les requêtes vont à l’API choisie ; votre Relay les fait transiter par votre serveur. Mémorisation des clés facultative dans ce navigateur.','Direkte Anfragen gehen an deine API; dein Relay leitet sie über deinen Server. Schlüssel können optional in diesem Browser gespeichert werden.'],
    'boundary.files':['文件属于你的设备','檔案屬於你的裝置','Files live on your device','ファイルは自分のデバイスに','파일은 내 기기에','Les fichiers restent sur l’appareil','Dateien auf deinem Gerät'],
    'boundary.files.detail':['持久工作区保存在本地 OPFS，临时 /tmp 与之分离。清除网站数据会删除工作区，请及时导出 ZIP。','持久工作區儲存在本機 OPFS，暫存 /tmp 與之分離。清除網站資料會刪除工作區，請及時匯出 ZIP。','Persistent workspaces use local OPFS, separate from temporary /tmp. Clearing site data deletes workspaces. Export ZIP backups.','永続ワークスペースはローカル OPFS、一時 /tmp とは別です。サイトデータの削除で失われるため、ZIP でバックアップを。','영구 작업 공간은 로컬 OPFS를 사용하며 임시 /tmp와 분리됩니다. 사이트 데이터를 지우면 삭제되므로 ZIP으로 백업하세요.','Les espaces persistants utilisent OPFS local, séparément de /tmp. Effacer les données du site les supprime : exportez des sauvegardes ZIP.','Persistente Workspaces nutzen lokales OPFS, getrennt von /tmp. Das Löschen der Websitedaten entfernt sie. Exportiere ZIP-Backups.'],
    'boundary.cors':['不隐藏浏览器限制','不隱藏瀏覽器限制','Browser limits, made explicit','ブラウザーの制限も明示','브라우저 제한도 명확하게','Les limites du navigateur, explicites','Browser-Grenzen offen benannt'],
    'boundary.cors.detail':['直连需要 API 支持 CORS，也可用自建中继或桌面扩展。PWA 不绕过 CORS；扩展不修复认证或 TLS 错误。','直連需要 API 支援 CORS，也可用自建中繼或桌面擴充功能。PWA 不會繞過 CORS；擴充功能不會修復驗證或 TLS 錯誤。','Direct mode needs API CORS support, your own relay, or a desktop extension. PWA does not bypass CORS. Extensions do not fix authentication or TLS.','直結には API の CORS 対応が必要です。自前 Relay やデスクトップ拡張も選べます。PWA は CORS を回避せず、拡張は認証や TLS を修復しません。','직접 연결에는 API의 CORS 지원이 필요합니다. 자체 중계나 데스크톱 확장도 가능합니다. PWA는 CORS를 우회하지 않으며 확장은 인증이나 TLS를 해결하지 않습니다.','Le mode direct nécessite CORS, votre relay ou une extension de bureau. Une PWA ne contourne pas CORS. L’extension ne corrige ni l’authentification ni TLS.','Direktmodus braucht API-CORS-Unterstützung, ein eigenes Relay oder eine Desktop-Erweiterung. PWA umgeht CORS nicht. Erweiterungen beheben weder Authentifizierung noch TLS.'],
    'downloads.title':['桌面 CORS 扩展与安装说明','桌面 CORS 擴充功能與安裝說明','Desktop CORS extensions and setup','デスクトップ CORS 拡張と導入','데스크톱 CORS 확장 및 설치','Extensions CORS et installation','Desktop-CORS-Erweiterungen und Installation'],
    'downloads.firefox':['Firefox 开发包 ↓','Firefox 開發套件 ↓','Firefox development ZIP ↓','Firefox 開発用 ZIP ↓','Firefox 개발 ZIP ↓','ZIP de développement Firefox ↓','Firefox-Entwickler-ZIP ↓'],
    'downloads.safari':['Safari 源码 ZIP ↓','Safari 原始碼 ZIP ↓','Safari source ZIP ↓','Safari ソース ZIP ↓','Safari 소스 ZIP ↓','ZIP source Safari ↓','Safari-Quellcode-ZIP ↓'],
    'downloads.guide':['安装步骤与验证范围 →','安裝步驟與驗證範圍 →','Setup and verification scope →','導入手順と検証範囲 →','설치 및 검증 범위 →','Installation et portée des tests →','Installation und Prüfumfang →'],
    'final.title':['少一点配置。多一点创造。','少一點設定。多一點創造。','Less setup. More making.','設定は少なく。創造は多く。','설정은 줄이고. 창작은 더 많이.','Moins de réglages. Plus de création.','Weniger einrichten. Mehr erschaffen.'],
    'final.description':['打开工作台，连接模型，从你的第一个任务开始。','開啟工作台，連接模型，從你的第一個任務開始。','Open a workspace, connect a model, and start with your first task.','ワークスペースを開き、モデルを接続して、最初のタスクへ。','작업 공간을 열고 모델을 연결해 첫 작업을 시작하세요.','Ouvrez un espace, connectez un modèle et commencez votre première tâche.','Öffne einen Workspace, verbinde ein Modell und starte deine erste Aufgabe.'],
    'docs.note':['首页与导航支持多语言；技术文档目前以中文为主。','首頁與導覽支援多語言；技術文件目前以中文為主。','Home and navigation are multilingual. Technical docs are currently mostly in Chinese.','ホームとナビゲーションは多言語対応。技術ドキュメントは現在主に中国語です。','홈과 탐색은 다국어를 지원합니다. 기술 문서는 현재 주로 중국어입니다.','L’accueil et la navigation sont multilingues. La documentation technique est surtout en chinois.','Startseite und Navigation sind mehrsprachig. Technische Dokumente sind derzeit überwiegend auf Chinesisch.'],
    'page.title':['i · lninl — 小核心，大作为。','i · lninl — 小核心，大作為。','i · lninl — Small core. Big moves.','i · lninl — 小さなコア、大きな可能性。','i · lninl — 작은 코어, 큰 가능성.','i · lninl — Petit noyau. Grandes idées.','i · lninl — Kleiner Kern. Viel bewegen.']
};
export const catalogs=Object.fromEntries(languages.map((language,index)=>[language,Object.fromEntries(Object.entries(messages).map(([key,row])=>{
    if(row.length!==languages.length||row.some(value=>typeof value!=='string'||!value.trim()))throw new Error(`Incomplete website translation: ${key}`);
    return [key,row[index]];
}))]));
export function detectSiteLanguage(preferences=[]){
    for(const preference of preferences){
        const tag=String(preference).toLowerCase().replaceAll('_','-');
        if(tag==='zh'||tag.startsWith('zh-')){
            if(tag.includes('-hans'))return 'zh-CN';if(tag.includes('-hant'))return 'zh-TW';
            return /-(tw|hk|mo)(-|$)/u.test(tag)?'zh-TW':'zh-CN';
        }
        const base=tag.split('-')[0];if(languages.includes(base))return base;
    }
    return 'en';
}
export function resolveSiteLanguage(saved,preferences){return languages.includes(saved)?saved:detectSiteLanguage(preferences);}
export function translate(language,key){return (catalogs[language]??catalogs.en)[key]??key;}
