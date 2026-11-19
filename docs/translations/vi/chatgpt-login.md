# Quản lý phần bổ trợ đăng nhập bằng i

> **Phí sử dụng Claude được tính qua API Console, không trừ vào hạn mức gói Pro hoặc Max.**
> Đăng nhập mà không nhập khóa API không có nghĩa là yêu cầu được miễn phí. Hãy kiểm tra tổ chức, không gian làm việc và quyền thanh toán trong Console trước khi cấp quyền.

Sử dụng `i` để quản lý kết nối mà không cần phải nhớ tên các file thực thi trợ lý hoặc nhập cài đặt kết nối thủ công. Các plugin đăng nhập gốc chạy với quyền của người dùng hệ điều hành. Chúng không phải là plugin công cụ WASM cho không gian làm việc; hãy sử dụng gói cài đặt chính xác.

## Thiết lập trong cửa sổ dòng lệnh

Bắt đầu bằng `i` và nhập lệnh quản lý cục, ngay cả khi bạn chưa cấu hình mô hình nào:

```text
/plugins
```

Chọn ChatGPT hoặc Claude, xem xét các quyền, sau đó cài đặt và kích hoạt tiện ích mở rộng. Nhập `/login`, chọn tiện ích mở rộng đăng nhập và ủy quyền nó trong trình duyệt hệ thống của bạn. Chọn một mô hình từ danh sách hoặc nhập ID mô hình mà bạn có thể sử dụng. Sau khi thành công, `i` sẽ lưu kết nối cho nhiệm vụ tiếp theo của bạn. Bạn cũng có thể nhập các lệnh trực tiếp:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Sử dụng `/cancel` để đóng menu. Bạn không thể thay đổi plugin hoặc kết nối trong khi một tác vụ đang chạy. Yêu cầu xác thực và danh sách mô hình không gửi yêu cầu tạo. Thay đổi kết nối chỉ xảy ra sau khi cả đăng nhập và chọn mô hình thành công. Các lỗi và hủy tác vụ vẫn giữ nguyên kết nối trước đó, và `i` lưu một bản sao lưu cấu hình riêng tư trước khi chuyển đổi.

## Quản lý phần bổ trợ bằng dòng lệnh

### Đổi mô hình và mức độ suy luận

`/model` đọc danh mục mô hình từ plugin kết nối gốc hiện tại. Chọn một số từ danh sách hoặc nhập trực tiếp ID mô hình. Các kết nối API-key tiêu chuẩn không truy vấn danh mục; bạn cần nhập ID mô hình. Điều này không yêu cầu đăng nhập lại hoặc chuyển đổi tài khoản:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Chỉ thay đổi cài đặt khi không có công việc, phê duyệt hoặc các mục đang chờ xử lý. Cài đặt được lưu trữ cho mỗi kết nối, với bản sao lưu riêng tư trước khi ghi. Hủy bỏ danh sách mô hình sẽ giữ nguyên cấu hình. Chỉ chuyển đổi mô hình sẽ đặt lại nỗ lực về giá trị mặc định của nhà cung cấp, do đó bạn sẽ không chuyển giao cài đặt của một mô hình khác. `auto` và `default` sẽ không chỉ định nỗ lực. Các giá trị khác là `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, và `max`. Không phải tất cả các mô hình đều hỗ trợ tất cả các giá trị, và danh mục có thể không quảng cáo khả năng của chúng. Thay đổi cài đặt không gửi yêu cầu tạo hóa tính phí. Nhà cung cấp sẽ xác minh hỗ trợ trong các yêu cầu tiếp theo. API Responses dùng `reasoning.effort`, còn API Chat Completions dùng `reasoning_effort`. Claude sử dụng `output_config.effort` với tư duy thích ứng. Giá trị `none` yêu cầu tắt tư duy trên các mô hình hỗ trợ chế độ này. Không nên giả định rằng tất cả các mô hình đều hỗ trợ nỗ lực cao hơn hoặc tắt tư duy. Xem [tư duy của OpenAI](https://developers.openai.com/api/docs/guides/reasoning) và [nỗ lực của Claude](https://platform.claude.com/docs/en/build-with-claude/effort).

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

Chuyển sang một tài khoản hiện có bằng cách sử dụng `i plugins use chatgpt ACCOUNT_ID`. Sử dụng `i login chatgpt --account ACCOUNT_ID` để xác thực lại một đăng ký hiện có, hoặc `i plugins logout chatgpt --account ACCOUNT_ID` để đăng xuất khỏi một tài khoản cụ thể. Quá trình xác thực lại thực hiện đăng ký mà không sao chép các token cũ. Nó không thay đổi tài khoản trên kết nối hiện tại cho đến khi bạn xác nhận mô hình.

Để quản lý phần bổ trợ Claude Console, hãy dùng `claude` thay cho `chatgpt`. Việc cài đặt và cập nhật yêu cầu xác nhận. Sau khi cập nhật, hãy bật lại tiện ích mở rộng và đăng nhập để tránh thay thế một kết nối đang hoạt động một cách lặng lẽ. Nếu bạn chỉ cập nhật tiện ích hỗ trợ và muốn giữ nguyên tài khoản, hãy bật lại tiện ích mở rộng và chạy lệnh `i plugins reconnect chatgpt`. Điều này giữ nguyên tài khoản và cài đặt mô hình hiện tại, đồng thời liên kết kết nối đang hoạt động với trình thực thi mới đã xác minh. Nó không đăng nhập lại hoặc gửi yêu cầu tạo. Trong quá trình cài đặt, `i` hiển thị tìm kiếm phiên bản, số lượng byte đã tải xuống, xác minh checksum và lưu tiến trình. Sử dụng `/cancel` để dừng. Kết nối sẽ hết thời gian sau 10 giây và mỗi lần tải xuống có thể mất đến 120 giây. Các lỗi không kích hoạt việc thử lại tự động hoặc bật tiện ích mở rộng. Việc tải xuống từ GitHub sử dụng các biến môi trường proxy của terminal, chứ không phải cài đặt proxy của trình duyệt. Nếu cần, hãy bắt đầu với `HTTPS_PROXY=http://127.0.0.1:3067 i`, thay thế địa chỉ và cổng bằng cài đặt proxy của bạn.

Bạn cũng có thể thay đổi các quy tắc proxy từ bên trong `i`. Theo mặc định, các quy tắc được lưu trữ cho từng người dùng trong `~/.i/config/proxy.json` (nếu `I_CONFIG_DIR` được thiết lập), chứ không phải trong không gian làm việc.

```text
/proxy
/proxy 3067
/proxy :3067
/proxy 127.0.0.1:3067
/proxy socks5://127.0.0.1:3067
/proxy no
/proxy env
/proxy chatgpt 3067
/proxy claude no
/proxy plugins no
/proxy model 3067
/proxy chatgpt default
```

Quy tắc áp dụng theo thứ tự sau: quy tắc theo mục đích, cài đặt mặc định của người dùng, sau đó môi trường hoặc cấu hình ban đầu. Một quy tắc `no` không bao giờ trở lại môi trường proxy. Các thay đổi chỉ ảnh hưởng đến các thao tác sau; chúng không làm gián đoạn hoặc lặp lại các yêu cầu đang hoạt động. Cấu hình trình duyệt và các dịch vụ độc lập đang chạy riêng biệt. Hỗ trợ HTTP, HTTPS, SOCKS5 và SOCKS5h. Các cuộc gọi xác thực cục bộ không sử dụng proxy. Các URL proxy không thể chứa tên người dùng, mật khẩu, truy vấn hoặc đường dẫn, do đó thông tin đăng nhập không xuất hiện trong lịch sử lệnh. Đây là lệnh của máy chủ; nó không tiết lộ cài đặt proxy hoặc thông tin đăng nhập tài khoản cho các plugin WASM trong không gian làm việc. Xóa lựa chọn cài đặt vẫn giữ lại các gói và thông tin đăng nhập cũ và không thu hồi xác thực từ xa. Để đăng xuất, hãy chạy `i plugins logout NAME` trước khi vô hiệu hóa hoặc xóa plugin. Nếu việc thu hồi từ xa không được xác nhận, hãy kiểm tra cài đặt tài khoản của nhà cung cấp.

Ứng trình tải xuống tệp kết nối phù hợp từ kho GitHub Release chính thức. Nó kiểm tra SHA-256 do GitHub cung cấp, danh sách các tệp ZIP được phép, và nền tảng đích, sau đó xác minh tính toàn vẹn của tệp thực thi trước mỗi lần gọi. Nếu không có nền tảng nào phù hợp, ứng trình sẽ dừng lại thay vì chạy các tập lệnh cài đặt của bên thứ ba. Kiểm tra tính toàn vẹn xác minh tính toàn vẹn, không phải danh tính của nhà xuất bản. Các gói **kết nối** dành cho macOS không được ký hoặc chứng nhận bởi Developer ID. Việc cài đặt plugin không thay đổi PATH hoặc yêu cầu quyền quản trị.

Nhà phát triển có thể kiểm tra gói phần mềm đã được xây dựng cục bộ bằng `i plugins install chatgpt --package /absolute/path/package.zip`. Sau khi xác nhận, trình cài đặt vẫn kiểm tra các thành phần của tệp và nền tảng đích. Plugin sẽ khởi động ở trạng thái tắt, và thông tin kiểm tra tính toàn vẹn của nó sẽ được kiểm tra trước khi sử dụng. Một gói phần mềm cục bộ được đánh dấu là `local-build`, và chưa được xác nhận là phiên bản chính thức. Chỉ cài đặt mã nguồn mà bạn tin tưởng. Xác nhận từ trình duyệt không có nghĩa là quá trình đăng nhập đã hoàn tất. Hãy chờ cho đến khi trình điều khiển hoàn tất việc trao đổi token, xác thực danh tính và chọn mô hình.

Các tệp `desktop.json` ở cấp người dùng cũ có thể vẫn liệt kê các mục `search` và `find_files`, những mục này hiện đã được tích hợp vào lõi. Khi khởi động, trình quản lý sẽ loại bỏ các mục tìm kiếm tích hợp cũ và liên kết các worker tích hợp cũ từ cùng một thư mục cấu hình người dùng với chương trình thực thi hiện tại, mà không thay đổi tệp trên ổ đĩa. Các tên trùng lặp cho các worker và plugin tùy chỉnh vẫn gây ra lỗi; trình quản lý không bỏ qua các kiểm tra trùng lặp. Các lỗi này xảy ra trước khi thực hiện yêu cầu mô hình, vì vậy việc đăng nhập lại là không cần thiết.

### Chẩn đoán lỗi khi tạo nội dung

Kết nối ChatGPT gốc sử dụng API Responses với `store:false` và `stream:true`, chứ không phải Chat Completions. Terminal ưu tiên SSE, nhưng cũng chấp nhận đối tượng Responses đầy đủ `application/json` nếu có `object:response`, ID hợp lệ, `status:completed` và đầu ra hợp lệ. Nếu không có header `Content-Type`, nó phát hiện SSE hoặc JSON từ nội dung và sau đó áp dụng cùng một quy trình xác thực. Nó từ chối HTML hoặc các loại phương tiện khác được đánh dấu rõ ràng. Chẩn đoán lỗi chỉ hiển thị danh mục định dạng và số byte, chứ không phải nội dung phản hồi. HTML, đối tượng Chat Completions, JSON không hợp lệ và các cuộc gọi công cụ không đầy đủ không được chấp nhận. Thay đổi định dạng phản hồi không gây ra việc gửi lại yêu cầu. Dịch vụ web cục bộ chuyển tiếp SSE hợp lệ. Nếu loại nội dung bị thiếu hoặc phản hồi là JSON, nó xác thực phản hồi đầy đủ trước khi cung cấp SSE hợp lệ cho trình duyệt. Con đường tương thích này chờ đợi cho đến khi hoàn thành và không tự động gửi lại. Một HTTP 400 cục bộ trong trình duyệt không nhất thiết phải là trạng thái từ xa: chẩn đoán phân biệt các lỗi HTTP từ xa, sự cố mạng và lỗi định dạng.

Trong trường hợp lỗi, công cụ chẩn đoán sẽ xác định giai đoạn: chuẩn bị yêu cầu, xác thực, HTTP, định dạng phản hồi hoặc phân tích luồng. Chẩn đoán HTTP chỉ bao gồm trạng thái, mã lỗi đã biết hoặc tên tham số, và một ID yêu cầu hạn chế. Chúng không hiển thị các lỗi, thông báo hoặc token thô. Lỗi mạng hoặc định dạng không xóa thông tin đăng nhập của bạn hoặc tự động chuyển đổi giữa các tài khoản, mô hình hoặc đường dẫn thanh toán API.

## Cấu hình và thông tin xác thực

- `<config_dir>/native-plugins.json` lưu trữ đường dẫn cài đặt, thông tin xác thực và trạng thái kích hoạt, chứ không phải các token OAuth.
- `<data_dir>/connections/accounts/NAME` lưu trữ thông tin xác thực của plugin; các plugin WASM cho không gian làm việc không nhận được thông tin này.
- Menu đăng nhập sẽ tự động điền cấu hình mô hình. Bạn không cần phải viết file `chatgpt.json`.
- macOS và Linux sử dụng các thư mục và quyền file riêng. Windows kế thừa quyền kiểm soát truy cập (ACL) của thư mục người dùng.
- Ứng dụng không mã hóa thông tin đăng nhập hoặc lưu trữ chúng trong hệ thống khóa. Các chương trình gốc độc hại đang chạy với cùng một tài khoản người dùng vẫn có thể đọc chúng.

Không nên đặt các thư mục này trong không gian làm việc, thư mục chung, hệ thống kiểm soát phiên bản hoặc bản sao lưu công khai. Kết nối API-key trực tiếp vẫn hoạt động mà không cần plugin đăng nhập.

## Ghép nối với trình duyệt

Sau khi cài đặt và kích hoạt tiện ích, hãy chạy lệnh này trên cùng một máy tính với trình duyệt:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Với Claude, hãy dùng `claude` thay cho `chatgpt` trong lệnh trên. Giữ terminal hoạt động và mở liên kết ghép đôi đã in. Xác nhận plugin, tài khoản và mô hình trên trang, sau đó lưu. Quá trình ghép đôi sẽ tạo ra một bản nháp chưa lưu và không tự động thay thế kết nối hiện có. Sử dụng liên kết mới sau khi khởi động lại dịch vụ. Không chia sẻ liên kết ghép đôi: mỗi liên kết chứa một bí mật khả năng cục bộ.

Dịch vụ chỉ hoạt động trên địa chỉ `127.0.0.1`, được liên kết với trang web cụ thể và không sử dụng để chuyển tiếp các URL bất kỳ. Các trang HTTPS truy cập vào địa chỉ loopback có thể cần quyền truy cập mạng cục bộ. Quá trình ghép đôi không bỏ qua các quyền của trình duyệt hoặc CORS. Một thiết bị di động không thể tự chạy plugin gốc, và việc xóa kết nối trình duyệt không đồng nghĩa với việc đăng xuất khỏi tài khoản gốc.

## Giới hạn của ChatGPT và cách khắc phục sự cố

Plugin sử dụng đăng ký khách hàng động theo OAuth chính thức, PKCE, định danh máy chủ riêng và lưu trữ thông tin đăng nhập riêng. Nó không đọc tệp `~/.codex/auth.json` cá nhân của bạn hoặc sử dụng các định danh khách hàng độc quyền. Vui lòng tham khảo [tài liệu hướng dẫn đăng nhập chính thức](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) và [hồ sơ và phiên](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

Inference sử dụng API công cộng Responses, chứ không phải API `backend-api` riêng. OpenAI xác định điều kiện đủ, chính sách không gian làm việc và giới hạn ủy quyền; đăng nhập không đảm bảo quyền truy cập Inference. Ổn định dựa trên đăng ký sử dụng cài đặt cố định là `store: false` và `stream: true`. Phiên bản thử nghiệm hiện tại bỏ qua cài đặt giới hạn số lượng token đầu ra; xem [hạn chế của phiên bản thử nghiệm](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Các lỗi bao gồm trao đổi mã xác thực, làm mới phiên và lỗi mạng. Chẩn đoán hiển thị mã trạng thái HTTP mà không hiển thị phản hồi từ điểm cuối mã, thông tin xác thực hoặc mã xác thực. Việc xác thực hoặc yêu cầu mô hình bị lỗi không được tự động lặp lại. Nếu việc trao đổi mã bị từ chối, hãy thử đăng nhập lại và kiểm tra phiên bản, đồng hồ hệ thống, mạng và cài đặt trình duyệt. Chỉ mã trạng thái không cho bạn biết lý do tại sao nó bị từ chối.

## Giới hạn của Claude Console

Làm theo hướng dẫn cài đặt [chính thức](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) để cài đặt `ant` và đảm bảo nó nằm trong biến môi trường PATH. Gói kết nối không bao gồm `ant` cũng không tự động chạy script cài đặt cho bạn. Sau đó, sử dụng lệnh `i login claude`.

Plugin sử dụng một môi trường HOME riêng, `ANTHROPIC_CONFIG_DIR`, và hồ sơ của riêng nó. Nó không đọc các Claude Code hoặc hồ sơ ant hiện có. Plugin hiện đang hỗ trợ một hồ sơ Console. Việc xác thực lại có thể thay đổi không gian làm việc, và các yêu cầu tiếp theo có thể sử dụng không gian làm việc mới; bắt đầu một phiên mới nếu cần. **Phí sử dụng Claude được tính qua API Console, không trừ vào hạn mức gói Pro hoặc Max.**

## Phạm vi kiểm chứng

Các bài kiểm tra tự động sử dụng các thư mục riêng biệt và các thiết lập OAuth/TLS cục bộ. Chúng không sử dụng các token cá nhân thực, không ủy quyền truy cập thay bạn, cũng không gửi các yêu cầu thanh toán. Việc vượt qua các bài kiểm tra ngoại tuyến không chứng minh rằng một nhà cung cấp sẽ cấp quyền truy cập vào một tài khoản thực. Các công cụ vẫn giữ nguyên các giao thức nội bộ của người dùng và dịch vụ; các thao tác hiển thị cho người dùng diễn ra thông qua `i`.
