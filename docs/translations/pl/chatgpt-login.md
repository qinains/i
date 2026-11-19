# Zarządzanie wtyczkami logowania za pomocą i

> **Korzystanie z Claude jest rozliczane przez Console API, a nie z limitu subskrypcji Pro lub Max.**
> Logowanie bez podawania klucza API nie sprawia, że żądania są bezpłatne. Przed udzieleniem dostępu sprawdź w Console organizację, obszar roboczy oraz uprawnienia do rozliczeń.

Użyj `i` do zarządzania połączeniami, bez konieczności zapamiętywania nazw wykonywalnych lub wpisywania ustawień połączenia ręcznie. Wtyczki do logowania działają z uprawnieniami użytkownika systemu operacyjnego. Nie są to wtyczki narzędziowe dla WASM w środowisku pracy; użyj odpowiedniego pakietu instalacyjnego.

## Konfiguracja w terminalu

Uruchom `i` i wprowadź polecenie zarządzania lokalnym, nawet jeśli jeszcze nie skonfigurowałeś żadnego modelu:

```text
/plugins
```

Wybierz ChatGPT lub Claude, przejrzyj uprawnienia, a następnie zainstaluj i włącz wtyczkę. Wpisz `/login`, wybierz wtyczkę logowania i autoryzuj ją w przeglądarce systemu. Wybierz model z listy lub wpisz identyfikator modelu, którego możesz użyć. Po pomyślnym wykonaniu, `i` zapisuje połączenie do następnego zadania. Możesz również wprowadzać polecenia bezpośrednio:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Użyj `/cancel`, aby zamknąć menu. Nie można zmieniać wtyczek ani połączeń, gdy zadanie jest w toku. Operacje autoryzacji i wyświetlania modeli nie wysyłają żądania generowania. Zmiany połączeń mają miejsce dopiero po pomyślnym zalogowaniu i wybraniu modelu. Błędy i anulowanie zachowują poprzednie połączenie, a `i` zapisuje prywatną kopię zapasową konfiguracji przed przełączeniem.

## Zarządzanie wtyczkami z wiersza poleceń

### Zmiana modelu i poziomu rozumowania

`/model` odczytuje katalog modeli z bieżącego wtyczki połączenia. Wybierz numer z listy lub wprowadź identyfikator modelu bezpośrednio. Standardowe połączenia z kluczem API nie wyszukują katalogu; wprowadź identyfikator modelu samodzielnie. Nie wymaga ponownego zalogowania ani zmiany kont:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Zmieniaj ustawienia tylko wtedy, gdy nie ma żadnych niezałatwionych zadań, zatwierdzeń ani elementów w kolejce. Ustawienia są zapisywane dla każdego połączenia, z prywatnym kopią zapasową przed zapisem. Anulowanie listy modeli pozostawia konfigurację bez zmian. Włączenie tylko jednego modelu przywraca wysiłek do domyślnych ustawień dostawcy, dzięki czemu nie przenosisz ustawień innego modelu. `auto` i `default` pozostawiają wysiłek bez określenia. Inne wartości to `none`, `minimal`, `low`, `medium`, `high`, `xhigh` i `max`. Nie wszystkie modele obsługują wszystkie wartości, a katalog może nie informować o ich możliwościach. Zmiana ustawienia nie wysyła żądania o wygenerowanie faktury. Dostawca weryfikuje wsparcie w kolejnych żądaniach. W interfejsie Responses API używa się `reasoning.effort`, a w interfejsie Chat Completions API — `reasoning_effort`. Claude używa `output_config.effort` z adaptacyjnym myśleniem. `none` prosi o wyłączenie myślenia w modelach, które obsługują ten tryb. Nie zakładaj, że każdy model obsługuje wyższy wysiłek lub wyłączenie myślenia. Zobacz [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) i [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort).

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

Przełącz się na istniejący konta, używając polecenia `i plugins use chatgpt ACCOUNT_ID`. Użyj polecenia `i login chatgpt --account ACCOUNT_ID` aby ponownie zarejestrować istniejące konto, lub `i plugins logout chatgpt --account ACCOUNT_ID` aby wylogować się z określonego konta. Proces reautoryzacji rejestruje konto bez kopiowania starych, obrotowych tokenów. Nie zmienia konta w bieżącej sesji, dopóki nie potwierdzisz modelu.

Aby zarządzać wtyczką Claude Console, użyj `claude` zamiast `chatgpt`. Instalacja i aktualizacje wymagają potwierdzenia. Po aktualizacji, ponownie włącz wtyczkę i zaloguj się, aby uniknąć cichego zastąpienia aktywnej połączenia. Jeśli aktualizujesz tylko pomocnika i chcesz zachować ten sam kont, ponownie włącz wtyczkę i uruchom polecenie `i plugins reconnect chatgpt`. Pozwoli to zachować obecny kont i ustawienia modelu oraz powiązać aktywne połączenie z nowo zweryfikowanym plikiem wykonywalnym. Nie wymaga ponownego zalogowania ani wysyłania żądania generowania. Podczas instalacji, `i` wyświetla informacje o wersji, pobranych bajtach, weryfikacji sum kontrolnych oraz zapisuje postęp. Użyj `/cancel`, aby zatrzymać. Połączenie wygasa po 10 sekundach, a każdy pobór może trwać do 120 sekund. Błędy nie powodują automatycznego ponownego próby ani włączają wtyczkę. Pobieranie z GitHub wykorzystuje zmienne środowiskowe terminala, a nie ustawienia proxy w przeglądarce. W razie potrzeby, rozpocznij od `HTTPS_PROXY=http://127.0.0.1:3067 i`, zastępując adres i port własnymi ustawieniami proxy.

Można również zmieniać reguły proxy bezpośrednio w `i`. Domyślnie, reguły są zapisywane dla każdego użytkownika w pliku `~/.i/config/proxy.json` (pod katalogiem `I_CONFIG_DIR`, jeśli jest ustawiony), a nie w środowisku roboczym.

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

Zasady obowiązują w następującej kolejności: zasada dla konkretnego celu, domyślne ustawienia użytkownika, a następnie oryginalne środowisko lub konfiguracja. Wyraźne polecenie `no` nigdy nie wraca do środowiska proxy. Zmiany dotyczą tylko późniejszych operacji; nie przerywają ani nie odtwarzają aktywnych żądań. Konfiguruj przeglądarki i już działające samodzielne usługi oddzielnie. Obsługiwane są protokoły HTTP, HTTPS, SOCKS5 i SOCKS5h. Lokalne wywołania zwrotne autoryzacji nie wykorzystują proxy. Adresy URL proxy nie mogą zawierać nazw użytkowników, haseł, zapytań ani ścieżek, więc dane uwierzytelniające nie znajdują się w historii poleceń. Jest to wbudowane polecenie hosta; nie ujawnia ustawień proxy ani danych uwierzytelniających dla wtyczek WASM w środowisku pracy. Usunięcie wyboru instalacji zachowuje stare pakiety i dane uwierzytelniające oraz nie odwołuje zdalnej autoryzacji. Aby się wylogować, uruchom polecenie `i plugins logout NAME` przed wyłączeniem lub usunięciem wtyczki. Jeśli zdalne odwołanie nie zostanie potwierdzone, sprawdź ustawienia konta u dostawcy.

Instalator pobiera odpowiedni plik instalacyjny ze oficjalnego repozytorium GitHub Release. Sprawdza SHA-256 dostarczony przez GitHub, listę dopuszczonych plików ZIP oraz docelową platformę, a następnie weryfikuje integralność pliku wykonywalnego przed każdym uruchomieniem. Jeśli nie zostanie znaleziona żadna odpowiednia platforma, instalacja zatrzymuje się, zamiast uruchamiać skrypty instalacyjne od innych źródeł. Sprawdzenie sum kontrolnej weryfikuje integralność, a nie tożsamość wydawcy. Pakiety instalacyjne dla macOS nie są podpisane przez Developer ID ani nie są notyfikowane. Instalacja wtyczki nie modyfikuje zmiennej PATH ani nie wymaga uprawnień administratora.

Deweloperzy mogą przetestować lokalnie zbudowany pakiet za pomocą polecenia `i plugins install chatgpt --package /absolute/path/package.zip`. Po potwierdzeniu, instalator nadal sprawdza zawartość archiwum oraz docelową platformę. Wtyczka jest uruchamiana w trybie wyłączonym, a przed użyciem sprawdzany jest jej plik wykonywalny. Lokalny pakiet ma etykietę `local-build` i nie jest uznawany za oficjalną wersję. Instaluj tylko kod, w którym masz zaufanie. Potwierdzenie w przeglądarce nie oznacza, że proces logowania zakończył się. Poczekaj, aż terminal zakończy wymianę tokenów, weryfikację tożsamości i wybór modelu.

Starsze pliki `desktop.json` na poziomie użytkownika mogą nadal zawierać wpisy dotyczące `search` i `find_files`, które teraz są zintegrowane z podstawowym systemem. Podczas ładowania, host usuwa rozpoznawalne stare, wbudowane wpisy wyszukiwania i łączy stare, wbudowane procesy z tego samego katalogu konfiguracji użytkownika z bieżącym wykonywalnym, bez zmiany pliku na dysku. Konfliktujące nazwy dla niestandardowych procesów i wtyczek nadal powodują błędy; host nie pomija sprawdzania duplikatów. Te błędy występują przed żądaniami modelu, więc logowanie ponownie nie jest konieczne.

### Diagnozowanie błędów generowania

Połączenia z natywnym ChatGPT wykorzystują API Responses z ustawieniami `store:false` i `stream:true`, a nie Chat Completions. Terminal preferuje SSE, ale również akceptuje kompletny obiekt `application/json` Responses, jeśli zawiera on `object:response`, poprawny ID, `status:completed` oraz poprawną zawartość. Bez nagłówka `Content-Type`, wykrywa SSE lub JSON z treści, a następnie stosuje tę samą walidację. Wyklucza wyraźnie oznaczone HTML lub inne nieobsługiwane typy mediów. Diagnozowanie błędów pokazuje tylko kategorię formatu i liczbę bajtów, a nie zawartość odpowiedzi. Nie są akceptowane obiekty HTML, Chat Completions, nieprawidłowe JSON oraz niekompletne wywołania narzędzi. Zmiana formatu odpowiedzi nie powoduje ponownego wysłania żądania. Lokalny serwer webowy przesyła poprawny SSE na żywo. Jeśli typ treści jest nieznany lub odpowiedź jest w formacie JSON, waliduje kompletną odpowiedź przed przekazaniem poprawnego SSE do przeglądarki. Ten ścieżka kompatybilności czeka na zakończenie i nie wysyła automatycznie. Lokalny błąd HTTP 400 w przeglądarce niekoniecznie oznacza błąd na serwerze: diagnostyka odróżnia błędy HTTP na serwerze, awarie sieci i błędy formatu.

W przypadku niepowodzenia, diagnostyka identyfikuje problem: przygotowanie żądania, autoryzację, format HTTP, format odpowiedzi lub parsowanie strumienia. Diagnostyka HTTP obejmuje jedynie status, znane kody błędów lub nazwy parametrów oraz ograniczony identyfikator żądania. Nie wyświetla surowych błędów, komunikatów ani tokenów. Błędy sieciowe lub formatowe nie powodują automatycznego wylogowania ani zmiany kont, modeli lub ścieżek rozliczeń kluczy API.

## Konfiguracja i dane uwierzytelniające

- `<config_dir>/native-plugins.json` przechowuje ścieżkę instalacji, sumę kontrolną oraz stan włączonej, a nie tokeny OAuth.
- `<data_dir>/connections/accounts/NAME` przechowuje dane uwierzytelniające dla wtyczki; wtyczki WASM dla przestrzeni roboczej nie otrzymują ich.
- Menu logowania wypełnia konfigurację modelu. Nie musisz pisać pliku `chatgpt.json`.
- macOS i Linux wykorzystują prywatne katalogi i uprawnienia do plików. Windows dziedziczy uprawnienia do katalogu użytkownika.
- Aplikacja nie szyfruje danych uwierzytelniających ani nie przechowuje ich w systemowym kluczu. Złośliwe programy działające jako ten sam użytkownik nadal mogą je odczytać.

Nie umieszczaj tych katalogów w przestrzeniach roboczych, współdzielonych folderach, systemach kontroli wersji ani w publicznych kopii zapasowych. Bezpośrednie połączenia z kluczami API nadal działają bez konieczności instalowania wtyczki logowania.

## Parowanie z przeglądarką

Po zainstalowaniu i włączeniu wtyczki, uruchom to na tym samym komputerze, na którym działa przeglądarka:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

W przypadku Claude wpisz w powyższym poleceniu `claude` zamiast `chatgpt`. Utrzymaj terminal włączony i otwórz wydrukowany link do sparowania. Potwierdź wtyczkę, konto i model na stronie, a następnie zapisz. Sparowanie tworzy początkowy, niezapisany projekt i nie zastępuje automatycznie istniejącej połączenia. Użyj nowego linku po ponownym uruchomieniu usługi. Nie udostępniaj linków do sparowania: każdy zawiera lokalny klucz autoryzacji.

Usługa działa tylko na adresie `127.0.0.1`, jest związana z określonym adresem strony i nie obsługuje arbitralnych URL-i. Strony HTTPS, które korzystają z połączenia "loopback", mogą wymagać uprawnień do sieci lokalnej. Parowanie nie omija uprawnień przeglądarki ani CORS. Telefon nie może samodzielnie uruchomić wbudowanego wtyczki, a usunięcie połączenia z przeglądarką nie powoduje wylogowania z wbudowanego konta.

## Ograniczenia ChatGPT i rozwiązywanie problemów

Wtyczka wykorzystuje oficjalną dynamiczną rejestrację klienta OAuth, PKCE, oddzielną tożsamość hosta oraz oddzielne przechowywanie danych uwierzytelniających. Nie odczytuje Twojego osobistego pliku `~/.codex/auth.json` ani nie wykorzystuje własnych identyfikatorów klienta. Zobacz [oficjalną dokumentację dotyczącą logowania](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) oraz [profilów i sesji](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

Funkcja wnioskowania wykorzystuje publiczny interfejs API Responses, a nie prywatny interfejs `backend-api`. OpenAI określa warunki dostępności, zasady przestrzeni roboczej oraz limity autoryzacji; logowanie nie gwarantuje dostępu do wnioskowania. Autoryzacja oparta na subskrypcji wykorzystuje stałe ustawienia `store: false` i `stream: true`. Aktualna wersja próbna ignoruje ustawienie maksymalnej liczby tokenów wyjściowych; szczegóły dotyczące ograniczeń wersji próbnej można znaleźć pod adresem [ograniczenia wersji próbnej](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Błędy dotyczą wymiany kodów autoryzacyjnych, odświeżania sesji oraz błędów sieciowych. Diagnoza pokazuje kody HTTP bez wyświetlania odpowiedzi z punktu końcowego tokena, danych uwierzytelniających lub kodów autoryzacyjnych. Nieautoryzowane lub błędne żądania nie są automatycznie ponawiane. Jeśli wymiana kodu zostanie odrzucona, spróbuj ponownie zalogować się i sprawdź wersję, zegar systemu, ustawienia sieci i proxy. Sam kod statusu nie mówi, dlaczego został odrzucony.

## Ograniczenia Claude Console

Postępuj zgodnie z [oficjalnym przewodnikiem instalacji](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart), aby zainstalować `ant` i upewnić się, że jest on w zmiennej środowiskowej PATH. Pakiet połączeniowy nie zawiera `ant` ani nie uruchamia jego skryptu instalacyjnego. Następnie użyj polecenia `i login claude`.

Wtyczka wykorzystuje oddzielne środowisko HOME, `ANTHROPIC_CONFIG_DIR`, oraz własny profil. Nie odczytuje istniejących profili Claude Code ani osobistych profili ant. Obecnie obsługuje jeden profil konsolowy. Ponowne autoryzowanie może zmienić przestrzeń roboczą, a kolejne żądania mogą korzystać z nowej przestrzeni roboczej; rozpocznij nową sesję, jeśli to konieczne. **Korzystanie z Claude jest rozliczane przez Console API, a nie z limitu subskrypcji Pro lub Max.**

## Zakres weryfikacji

Automatyczne testy wykorzystują odizolowane katalogi i lokalne konfiguracje OAuth/TLS. Nie wykorzystują rzeczywistych tokenów osobistych, nie autoryzują dostępu w Twoim imieniu ani nie wysyłają żądań, za które pobierane są opłaty. Pomyślne przeprowadzenie testów offline nie dowodzi, że dostawca przyzna dostęp do rzeczywistego konta. Narzędzia zachowują swoje wewnętrzne protokoły pracy i usług; operacje dostępne dla użytkownika przechodzą przez `i`.
