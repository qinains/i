# i ile oturum açma eklentilerini yönetme

> **Claude kullanımı Console API üzerinden ücretlendirilir; Pro veya Max abonelik kotasından düşmez.**
> API anahtarı girmeden oturum açmak istekleri ücretsiz yapmaz. Erişim izni vermeden önce Console’daki kuruluşu, çalışma alanını ve faturalandırma izinlerini kontrol edin.

`i` komutunu kullanarak bağlantıları yönetebilir, yardımcı programların adlarını veya bağlantı ayarlarını elle yazmak zorunda kalmadan. Yerel giriş eklentileri, işletim sisteminizin kullanıcı izinleriyle birlikte çalışır. Bunlar, çalışma alanı WASM araç eklentileri değildir; doğru kurulum paketini kullanın.

## Terminalden kurulum

`i` ile başlayın ve yerel yönetim komutunu girin, henüz bir model yapılandırmadıysanız bile:

```text
/plugins
```

ChatGPT veya Claude'ı seçin, izinleri kontrol edin, ardından eklentiyi yükleyin ve etkinleştirin. `/login` komutunu girin, bir giriş eklentisi seçin ve sistem tarayıcınızda yetkilendirin. Liste içinden bir model seçin veya kullanabileceğiniz bir model kimliği girin. Başarılı bir şekilde bağlandığında, `i` komutu, bir sonraki göreviniz için bağlantıyı kaydeder. Ayrıca komutları doğrudan da girebilirsiniz:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Menüyü kapatmak için `/cancel` komutunu kullanın. Bir görev sırasında eklentileri veya bağlantıları değiştiremezsiniz. Yetkilendirme ve model listeleme, oluşturma isteklerini göndermez. Bağlantı değişiklikleri yalnızca hem giriş hem de model seçimi başarılı olduktan sonra gerçekleşir. Başarısızlıklar ve iptaller, önceki bağlantıyı korur ve `i` anahtarı, geçiş yapmadan önce özel bir yapılandırma yedeklemesini kaydeder.

## Eklentileri komut satırından yönetme

### Modeli ve akıl yürütme düzeyini değiştirme

`/model` komutu, mevcut yerel bağlantı eklentisinden model kataloğunu okur. Liste içinden bir sayı seçin veya doğrudan bir model kimliğini girin. Standart API anahtarı bağlantıları, bir katalogdan sorgu yapmaz; model kimliğini kendiniz girmeniz gerekir. Bu, tekrar oturum açmayı veya hesap değiştirmeyi gerektirmez:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Ayarları yalnızca hiçbir görev, onay veya bekleyen öğe olmadığında değiştirin. Ayarlar, yazmadan önce özel bir yedekleme ile bağlantı başına kaydedilir. Model listesini iptal etmek, yapılandırmayı değiştirmez. Sadece modeli değiştirmek, çabayı sağlayıcının varsayılan değerine geri döndürür, böylece başka bir modelin ayarlarını taşımazsınız. `auto` ve `default` çabayı belirtmez. Diğer değerler `none`, `minimal`, `low`, `medium`, `high`, `xhigh` ve `max` şeklindedir. Tüm modellerin tüm değerleri desteklemediğini ve kataloğun yeteneklerini belirtmediğini unutmayın. Ayarın değiştirilmesi, ödeme talep eden bir istek göndermez. Responses API `reasoning.effort`, Chat Completions API ise `reasoning_effort` kullanır. Claude, adaptif düşünme ile birlikte `output_config.effort` değerini kullanır. `none` değeri, bu mod desteği olan modellerde düşünmenin devre dışı bırakılmasını talep eder. Her modelin daha yüksek çaba veya düşünmeyi devre dışı bırakma yeteneği olduğunu varsaymayın. [OpenAI düşünme](https://developers.openai.com/api/docs/guides/reasoning) ve [Claude çabası](https://platform.claude.com/docs/en/build-with-claude/effort) bölümlerine bakın.

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

`i plugins use chatgpt ACCOUNT_ID` komutu ile mevcut bir hesaba geçin. Mevcut bir kaydı yeniden yetkilendirmek için `i login chatgpt --account ACCOUNT_ID` komutunu kullanın veya belirli bir hesaptan çıkmak için `i plugins logout chatgpt --account ACCOUNT_ID` komutunu kullanın. Yeniden yetkilendirme, eski dönen token'ları kopyalamadan kaydı tamamlar. Modelin doğrulanana kadar mevcut bağlantıda hesabı değiştirmez.

Claude Console eklentisini yönetmek için `chatgpt` yerine `claude` kullanın. Kurulum ve güncellemeler onay gerektirir. Güncellemeden sonra, eklentiyi tekrar etkinleştirin ve bağlantıyı sessizce değiştirmemek için oturum açın. Sadece yardımcısını güncelleyip aynı hesabı korumak istiyorsanız, eklentiyi tekrar etkinleştirin ve `i plugins reconnect chatgpt` komutunu çalıştırın. Bu, mevcut hesabı ve model ayarlarını korur ve etkin bağlantıyı doğrulanmış yeni çalıştırılabilir dosyaya bağlar. Tekrar oturum açmaz veya bir oluşturma isteği göndermez. Kurulum sırasında, `i` sürüm arama, indirilen baytlar, kontrol toplamı doğrulama ve ilerleme kaydetme gibi işlevleri gösterir. Durdurmak için `/cancel` komutunu kullanın. Bağlantı 10 saniye sonra sona erer ve her indirme 120 saniye sürebilir. Başarısızlıklar otomatik yeniden deneme veya eklentiyi etkinleştirmeye neden olmaz. GitHub indirmeleri, tarayıcınızın proxy ayarlarının yerine, terminalin proxy ortam değişkenlerini kullanır. Gerekirse, `HTTPS_PROXY=http://127.0.0.1:3067 i` komutunu kullanarak, adres ve portu kendi proxy ayarlarınızla değiştirerek başlayın.

`i` içinde de proxy kurallarını değiştirebilirsiniz. Varsayılan olarak, kurallar `~/.i/config/proxy.json` içinde, ancak `I_CONFIG_DIR` altında (eğer ayarlanmışsa) kullanıcı başına kaydedilir, çalışma alanında değil:

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

Kurallar aşağıdaki sırayla uygulanır: amaç odaklı kural, kullanıcı varsayılan ayarlar, ardından orijinal ortam veya yapılandırma. Açık bir `no`, hiçbir zaman bir ortam aracına geri dönmez. Değişiklikler yalnızca sonraki işlemleri etkiler; mevcut istekleri kesmez veya tekrar oynatmaz. Tarayıcıları ve zaten çalışan bağımsız hizmetleri ayrı ayrı yapılandırın. HTTP, HTTPS, SOCKS5 ve SOCKS5h desteklenir. Yerel yetkilendirme çağrıları, aracı kullanmaz. Aracı URL'leri, kullanıcı adlarını, şifreleri, sorguları veya yolları içermemelidir, bu nedenle kimlik bilgileri komut geçmişine geçmez. Bu, yerleşik bir ana makine komutudur; aracı ayarlarını veya hesap kimlik bilgilerini, çalışma alanı WASM eklentilerine açmaz. Bir kurulum seçeneğini kaldırmak, eski paketleri ve kimlik bilgilerini korur ve uzaktan yetkilendirmeyi iptal etmez. Çıkış yapmak için, eklentiyi devre dışı bırakmadan veya kaldırmadan önce `i plugins logout NAME` komutunu çalıştırın. Uzaktan iptal edilip edilmediği doğrulanmazsa, sağlayıcının hesap ayarlarını kontrol edin.

Kurulum programı, resmi GitHub Yayınlarından eşleşen bağlantı dosyasını indirir. Ardından, GitHub tarafından sağlanan SHA-256 değerini, ZIP dosyasındaki izin listesini ve hedef platformu kontrol eder ve her çağrıda yürütülebilir dosyanın bütünlüğünü doğrular. Eğer herhangi bir platform eşleşmezse, üçüncü taraf kurulum betiklerini çalıştırmak yerine durur. Kontrol, yayıncının kimliğini doğrulamak yerine dosyanın bütünlüğünü doğrular. macOS **bağlantı paketleri**, geliştirici kimliği ile imzalanmamış veya onaylanmamıştır. Eklenti kurulumu, PATH'i değiştirmez veya yönetici ayrıcalıkları gerektirmez.

Geliştiriciler, `i plugins install chatgpt --package /absolute/path/package.zip` komutuyla yerel olarak oluşturulmuş bir paketi test edebilirler. Onaylandıktan sonra, kurulum programı yine de arşiv üyelerini ve hedef platformu kontrol eder. Eklenti başlangıçta devre dışıdır ve kullanmadan önce çalıştırılabilir dosyasının bütünlüğü kontrol edilir. Yerel bir paket, `local-build` olarak etiketlenir ve resmi bir sürüm olarak doğrulanmaz. Sadece güvendiğiniz yerel kodu kurun. Bir tarayıcı geri çağırma onayı, oturum açmanın tamamlandığını göstermez. Terminalin, token değişimini, kimlik doğrulamasını ve model seçimi işlemlerini tamamlamasını bekleyin.

Daha eski, kullanıcı düzeyindeki `desktop.json` dosyaları hala `search` ve `find_files` gibi öğeleri içerebilir, ancak bu özellikler artık temel işlevlere dahil edilmiştir. Uygulama başladığında, ana program, tanınabilir eski yerleşik arama girdilerini kaldırır ve aynı kullanıcı yapılandırma dizininden gelen eski yerleşik çalışanları mevcut yürütülebilir dosyaya bağlar, ancak dosyayı diskte değiştirmez. Özel çalışanlar ve eklentiler için çakışan isimler hala bir hata üretir; ana program, yinelenen kontrolleri atlamaz. Bu hatalar, model isteklerinden önce meydana geldiği için, tekrar oturum açmak gereksizdir.

### Üretim hatalarını tanılama

Yerel ChatGPT bağlantıları, `store:false` ve `stream:true` parametrelerini kullanarak Responses API'sini kullanır, Chat Completions'ı değil. Terminal, SSE'yi tercih etse de, `object:response`, geçerli bir ID, `status:completed` ve geçerli çıktı varsa, tam bir `application/json` Responses nesnesini de kabul eder. `Content-Type` başlığı olmadan, vücuttan SSE veya JSON'u algılar ve ardından aynı doğrulamayı uygular. Açıkça işaretlenmiş HTML veya diğer desteklenmeyen medya türlerini reddeder. Hata teşhisleri, yalnızca format kategorisini ve bayt sayısını gösterir, yanıtın içeriğini değil. HTML, Chat Completions nesneleri, geçersiz JSON ve eksik araç çağrıları kabul edilmez. Yanıt formatındaki değişiklik, bir isteğin yeniden gönderilmesine neden olmaz. Yerel web hizmeti, geçerli SSE'yi canlı olarak iletir. İçerik türü eksikse veya yanıt JSON ise, geçerli SSE'yi tarayıcıya vermeden önce yanıtın tamamını doğrular. Bu uyumluluk yolu, tamamlanmayı bekler ve otomatik olarak yeniden göndermez. Tarayıcıda yerel bir HTTP 400 hatası, mutlaka yukarı akış durumunu ifade etmez: Teşhisler, yukarı akış HTTP hatalarını, ağ arızalarını ve format hatalarını ayırır.

Başarısızlık durumunda, tanı aracı aşağıdaki aşamaları belirler: istek hazırlama, yetkilendirme, HTTP, yanıt formatı veya akış ayrıştırma. HTTP tanıları yalnızca durum, bilinen hata kodları veya parametre adlarını ve sınırlı bir istek kimliğini içerir. Ham hataları, ipuçlarını veya belirteçleri göstermez. Ağ veya format hataları, oturum açmanızı veya otomatik olarak hesaplarınızı, modelleri veya API anahtar ödeme yollarını değiştirmez.

## Yapılandırma ve kimlik bilgileri

- `<config_dir>/native-plugins.json`, OAuth token'leri değil, kurulum yolu, hash değeri ve etkin durum bilgilerini saklar.
- `<data_dir>/connections/accounts/NAME` dizini, eklentinin kimlik bilgilerini saklar; WASM çalışma alanı eklentileri bu kimlik bilgilerini almaz.
- Giriş menüsü, model yapılandırmasını otomatik olarak doldurur. `chatgpt.json` dosyasını kendiniz oluşturmanıza gerek yoktur.
- macOS ve Linux, özel dizinler ve dosya izinlerini kullanır. Windows, kullanıcı dizin ACL'sini devralır.
- Bu uygulama, kimlik bilgilerini şifrelemez veya bir sistem anahtarına kaydetmez. Aynı kullanıcı olarak çalışan kötü amaçlı yerel programlar, bu kimlik bilgilerini yine de okuyabilir.

Bu klasörleri çalışma alanlarına, paylaşılan klasörlere, sürüm kontrol sistemlerine veya kamuya açık yedeklemelere eklemeyin. Doğrudan API anahtar bağlantıları, giriş yapma eklentisine ihtiyaç duymadan çalışmaya devam ediyor.

## Tarayıcıyla eşleştirme

Eklentiyi kurduktan ve etkinleştirdikten sonra, aşağıdaki komutu aynı bilgisayarda tarayıcı üzerinden çalıştırın:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Claude için yukarıdaki komutta `chatgpt` yerine `claude` kullanın. Terminali açık tutun ve yazdığınız eşleştirme bağlantısını açın. Sayfadaki eklentiyi, hesabı ve modeli doğrulayın, ardından kaydedin. Eşleştirme işlemi, öncelikle kaydedilmemiş bir taslak oluşturur ve mevcut bir bağlantıyı otomatik olarak değiştiremez. Hizmeti yeniden başlatın ve yeni bir bağlantı kullanın. Eşleştirme bağlantılarını paylaşmayın: her biri yerel bir yetenek sırını içerir.

Bu hizmet yalnızca `127.0.0.1` adresinde çalışır, belirtilen sayfa kaynağına bağlanır ve rastgele URL'leri yönlendirmez. Döngü (loopback) üzerinden erişilen HTTPS sayfaları için yerel ağ izni gerekebilir. Eşleştirme, tarayıcı izinlerini veya CORS'u atlamaz. Bir telefon, kendi başına yerel eklentiyi çalıştıramaz ve bir tarayıcı bağlantısını silmek, yerel hesayı kapatmaz.

## ChatGPT sınırlamaları ve sorun giderme

Bu eklenti, resmi OAuth dinamik istemci kaydı, PKCE (Proof Key for Code Exchange), ayrı bir ana bilgisayar kimliği ve ayrı kimlik depolama yöntemlerini kullanır. Kişisel `~/.codex/auth.json` dosyanızı okumaz veya özel istemci kimliklerini kullanmaz. [Resmi oturum açma belgelerine](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) ve [profiller ve oturumlar](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions) bölümüne bakın.

Inference, `backend-api` yerine halka açık Responses API'sini kullanır. OpenAI, uygunluğu, çalışma alanı politikalarını ve yetkilendirme limitlerini belirler; giriş yapmak, Inference erişimini garanti etmez. Abonelik tabanlı yetkilendirme, `store: false` ve `stream: true` gibi sabit ayarları kullanır. Mevcut önizleme, maksimum çıktı token ayarını göz ardı eder; [önizleme kısıtlamalarını](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations) görüntüleyin.

Yetkilendirme kodu değişimi, oturum yenileme ve ağ hataları, sorunları ortaya çıkarır. Tanılama, ham token uç noktası yanıtlarını, kimlik bilgilerini veya yetkilendirme kodlarını göstermeden HTTP durum kodlarını görüntüler. Başarısız yetkilendirme veya model istekleri otomatik olarak tekrar denemez. Bir kod değişimi reddedildiğinde, tekrar giriş yapın ve sürüm, sistem saati, ağ ve proxy ayarlarını kontrol edin. Durum kodu tek başına, neden reddedildiğini size söylemez.

## Claude Console sınırlamaları

[Resmi kurulum kılavuzunu](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) takip ederek `ant`'i kurun ve PATH'te olduğundan emin olun. Bağlantı paketi, `ant`'i veya kurulum script'ini sizin için çalıştırmaz. Daha sonra `i login claude` komutunu kullanın.

Bu eklenti, `ANTHROPIC_CONFIG_DIR` adlı izole bir HOME alanı, kendi profili ve mevcut Claude Code veya kişisel ant profillerini okumaz. Şu anda yalnızca bir Console profiline destek vermektedir. Yeniden yetkilendirme, çalışma alanını değiştirebilir ve ardından gelen istekler yeni çalışma alanını kullanabilir; gerekirse yeni bir oturum başlatın. **Claude kullanımı Console API üzerinden ücretlendirilir; Pro veya Max abonelik kotasından düşmez.**

## Doğrulama kapsamı

Otomatik testler, izole edilmiş dizinler ve yerel OAuth/TLS yapılarını kullanır. Gerçek kişisel token'ları kullanmaz, sizin adınıza erişim yetkilendirme veya ücretli istek gönderme işlevini yerine getirmez. Çevrimdışı testlerin başarılı olması, bir sağlayıcının gerçek bir hesabı kullanıma açacağını kanıtlamaz. Yardımcı fonksiyonlar, kendi dahili çalışan ve hizmet protokollerini korur; kullanıcı arayüzünde gerçekleştirilen işlemler `i` üzerinden yapılır.
