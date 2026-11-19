# Anmelde-Plugins mit i verwalten

> **Die Nutzung von Claude wird über die Console API abgerechnet und nicht auf das Kontingent Ihres Pro- oder Max-Abos angerechnet.**
> Eine Anmeldung ohne Eingabe eines API-Schlüssels macht Anfragen nicht kostenlos. Prüfen Sie vor der Freigabe Organisation, Arbeitsbereich und Abrechnungsberechtigungen in der Console.

Verwenden Sie `i`, um Verbindungen zu verwalten, ohne die Namen von Hilfsprogrammen oder Verbindungs-Einstellungen manuell zu merken. Native Sign-in-Plugins laufen mit den Benutzerberechtigungen Ihres Betriebssystems. Sie sind keine Plugins für den Workspace-WASM-Tool; verwenden Sie das korrekte Installationspaket.

## Im Terminal einrichten

Starten Sie `i` und geben Sie den entsprechenden Verwaltungsbefehl ein, auch wenn Sie noch kein Modell konfiguriert haben:

```text
/plugins
```

Wählen Sie ChatGPT oder Claude, überprüfen Sie die Berechtigungen und installieren und aktivieren Sie dann den Plugin. Geben Sie `/login` ein, wählen Sie ein Anmelde-Plugin aus und autorisieren Sie es in Ihrem Systembrowser. Wählen Sie ein Modell aus der Liste oder geben Sie eine Modell-ID ein, die Sie verwenden können. Nach erfolgreicher Ausführung speichert `i` die Verbindung für Ihre nächste Aufgabe. Sie können die Befehle auch direkt eingeben:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Verwenden Sie `/cancel`, um das Menü zu schließen. Sie können keine Plugins oder Verbindungen ändern, während eine Aufgabe ausgeführt wird. Die Autorisierung und die Modellauflistung senden keine Generierungsanfragen. Die Verbindungsänderungen erfolgen erst, nachdem sowohl die Anmeldung als auch die Modellauswahl erfolgreich waren. Fehler und Abbruche hinterlassen die vorherige Verbindung intakt, und `i` speichert ein privates Konfigurationsbackup, bevor der Wechsel erfolgt.

## Plugins über die Befehlszeile verwalten

### Modell und Reasoning-Aufwand ändern

`/model` liest den Modellkatalog vom aktuellen nativen Verbindungspaket. Wählen Sie eine Zahl aus der Liste oder geben Sie eine Modell-ID direkt ein. Standard-API-Verbindungen fragen keinen Katalog ab; geben Sie die Modell-ID selbst ein. Dies bedeutet keine erneute Anmeldung oder Kontowechsel:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Ändern Sie die Einstellungen nur, wenn keine Aufgaben, Genehmigungen oder ausstehenden Elemente vorhanden sind. Die Einstellungen werden pro Verbindung gespeichert, wobei vor dem Schreiben ein privater Backup erstellt wird. Das Löschen der Modellliste ändert die Konfiguration nicht. Das Umschalten nur des Modells setzt den Aufwand auf den Standard des Anbieters zurück, sodass Sie die Einstellungen eines anderen Modells nicht übernehmen. `auto` und `default` lassen den Aufwand unbestimmt. Andere Werte sind `none`, `minimal`, `low`, `medium`, `high`, `xhigh` und `max`. Nicht alle Modelle unterstützen jeden Wert, und der Katalog bewirbt möglicherweise nicht ihre Fähigkeiten. Das Ändern der Einstellung sendet keine Rechnung für eine Generierung. Der Anbieter prüft die Unterstützung bei nachfolgenden Anfragen. Die Responses API verwendet `reasoning.effort`, die Chat Completions API verwendet `reasoning_effort`. Claude verwendet `output_config.effort` mit adaptiver Denkweise. `none` fordert, dass die Denkweise bei Modellen deaktiviert wird, die diese Funktion unterstützen. Nehmen Sie nicht an, dass jedes Modell eine höhere Anstrengung oder das Deaktivieren der Denkweise unterstützt. Sehen Sie sich [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) und [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort) an.

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

Wechseln Sie zu einem bestehenden Konto mit `i plugins use chatgpt ACCOUNT_ID`. Verwenden Sie `i login chatgpt --account ACCOUNT_ID`, um eine bestehende Registrierung neu zu autorisieren, oder `i plugins logout chatgpt --account ACCOUNT_ID`, um sich von einem bestimmten Konto abzumelden. Die Neuautorisierung ermöglicht die Registrierung, ohne alte, sich ändernde Token zu kopieren. Sie ändert die Verbindung nicht, bis Sie das Modell bestätigen.

Verwenden Sie für das Claude-Console-Plugin `claude` statt `chatgpt`. Die Installation und Updates erfordern eine Bestätigung. Nachdem Sie aktualisiert haben, aktivieren Sie das Plugin erneut und melden Sie sich an, um eine stille Ersetzung einer aktiven Verbindung zu vermeiden. Wenn Sie nur den Helfer aktualisieren und Ihr bestehendes Konto beibehalten möchten, aktivieren Sie das Plugin erneut und führen Sie `i plugins reconnect chatgpt` aus. Dadurch wird das aktuelle Konto und die Modell-Einstellungen beibehalten, und die aktive Verbindung wird an die verifizierte neue ausführbare Datei gebunden. Es wird nicht erneut angemeldet oder eine Generierungsanfrage gesendet. Während der Installation zeigt `i` Versionsabfrage, heruntergeladene Bytes, Überprüfung der Prüfsumme und Speichern des Fortschritts. Verwenden Sie `/cancel`, um zu stoppen. Die Verbindung wird nach 10 Sekunden beendet, und jeder Download kann bis zu 120 Sekunden dauern. Fehler lösen keine automatische Wiederholung oder aktivieren das Plugin. Downloads von GitHub verwenden die Proxy-Umgebungsvariablen des Terminals, nicht die Proxy-Einstellungen Ihres Browsers. Wenn erforderlich, beginnen Sie mit `HTTPS_PROXY=http://127.0.0.1:3067 i`, wobei Sie die Adresse und den Port durch Ihre eigenen Proxy-Einstellungen ersetzen.

Sie können die Proxy-Regeln auch innerhalb von `i` ändern. Standardmäßig werden die Regeln pro Benutzer in `~/.i/config/proxy.json` gespeichert (unter `I_CONFIG_DIR`, falls festgelegt), nicht im Arbeitsbereich.

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

Die Regeln gelten in folgender Reihenfolge: Regel für einen bestimmten Zweck, Standardeinstellungen des Benutzers, dann die ursprüngliche Umgebung oder Konfiguration. Eine explizite `no` kehrt niemals zu einer Umgebung zurück. Änderungen betreffen nur nachfolgende Operationen; sie unterbrechen oder wiederholen keine aktiven Anfragen. Konfigurieren Sie Browser und bereits laufende Standalone-Dienste separat. HTTP, HTTPS, SOCKS5 und SOCKS5h werden unterstützt. Lokale Autorisierungs-Callbacks verwenden keinen Proxy. Proxy-URLs dürfen keine Benutzernamen, Passwörter, Abfragen oder Pfade enthalten, sodass Anmeldeinformationen nicht in der Befehlszeile gespeichert werden. Dies ist ein eingebauter Host-Befehl; er stellt Proxy-Einstellungen oder Kontoverwaltungsdaten nicht für Workspace WASM-Plugins zur Verfügung. Das Entfernen einer Installationsauswahl behält alte Pakete und Anmeldeinformationen bei und widerruft keine Remote-Autorisierung. Um sich abzumelden, führen Sie `i plugins logout NAME` aus, bevor Sie das Plugin deaktivieren oder entfernen. Wenn die Remote-Widerrufung nicht bestätigt wurde, überprüfen Sie die Kontoeinstellungen des Anbieters.

Der Installer lädt die passende Verbindung zu dem offiziellen GitHub Release herunter. Er prüft den von GitHub bereitgestellten SHA-256-Hash, die ZIP-Dateien, die auf der Erlaubungsliste stehen, und die Zielplattform, und überprüft dann vor jeder Ausführung den Digest der ausführbaren Datei. Wenn keine Plattform übereinstimmt, wird der Installer gestoppt, anstatt Drittanbieter-Installationsskripte auszuführen. Der Checksummen-Vergleich stellt die Integrität sicher, nicht die Identität des Verliers. Die macOS **Verbindungspakete** sind nicht mit einer Developer ID signiert oder notifiziert. Die Installation des Plugins ändert den PATH nicht und erfordert keine Administratorrechte.

Entwickler können ein lokal erstelltes Paket mit `i plugins install chatgpt --package /absolute/path/package.zip` testen. Nach der Bestätigung überprüft der Installer weiterhin die Archivmitglieder und die Zielplattform. Das Plugin startet deaktiviert und seine ausführbare Datei wird vor der Verwendung überprüft. Ein lokales Paket wird mit `local-build` bezeichnet und ist nicht als offizielle Version bestätigt. Installieren Sie nur Code, dem Sie vertrauen. Eine Browser-Callback-Bestätigung bedeutet nicht, dass die Anmeldung abgeschlossen ist. Warten Sie, bis der Terminal-Prozess den Token-Austausch, die Identitätsverifizierung und die Modellauswahl abgeschlossen hat.

Ältere Dateien im Benutzerbereich, die `desktop.json` enthalten, können immer noch Einträge für `search` und `find_files` auflisten, die jetzt in den Kern integriert sind. Beim Start entfernt der Host erkennbare, alte, integrierte Suchfunktionen und bindet alte, integrierte Worker aus demselben Benutzerkonfigurationsverzeichnis an die aktuelle ausführbare Datei, ohne die Datei auf der Festplatte zu ändern. Konfliktierende Namen für benutzerdefinierte Worker und Plugins führen immer noch zu Fehlern; der Host führt keine doppelten Prüfungen aus. Diese Fehler treten vor Anfragen an das Modell auf, daher ist es nicht notwendig, sich erneut anzumelden.

### Fehler bei der Generierung diagnostizieren

Die nativen ChatGPT-Verbindungen verwenden die Responses API mit `store:false` und `stream:true`, nicht Chat Completions. Der Terminal bevorzugt SSE, akzeptiert aber auch ein vollständiges `application/json` Responses-Objekt, wenn es `object:response`, eine gültige ID, `status:completed` und gültige Ausgabe enthält. Ohne einen `Content-Type`-Header erkennt es SSE oder JSON aus dem Body und wendet dann die gleiche Validierung an. Es lehnt explizit markierte HTML- oder andere nicht unterstützte Medientypen ab. Die Fehlerdiagnose zeigt nur die Formatkategorie und die Byteanzahl, nicht den Antwortkörper. HTML, Chat Completions-Objekte, ungültiges JSON und unvollständige Tool-Aufrufe werden nicht akzeptiert. Eine Änderung des Antwortformats führt nicht dazu, dass eine Anfrage erneut gesendet wird. Der lokale Webdienst leitet gültiges SSE-Live weiter. Wenn der Medientyp fehlt oder die Antwort JSON ist, validiert er die vollständige Antwort, bevor er gültiges SSE an den Browser sendet. Dieser Kompatibilitätsweg wartet auf die Fertigstellung und sendet nicht automatisch erneut. Ein lokaler HTTP 400 im Browser bedeutet nicht unbedingt einen Fehler im Upstream: Die Diagnose unterscheidet zwischen Upstream-HTTP-Fehlern, Netzwerkfehlern und Formatfehlern.

Im Fehlerfall identifiziert die Diagnose den Fehlerzustand: Vorbereitung der Anfrage, Autorisierung, HTTP, Antwortformat oder Parsing des Datenstroms. Die HTTP-Diagnose umfasst nur den Status, bekannte Fehlercodes oder Parameterbezeichnungen sowie eine begrenzte Anfrage-ID. Sie zeigen keine Rohfehler, Aufforderungen oder Tokens an. Fehler im Netzwerk oder im Format führen nicht dazu, dass Ihr Anmeldeverhalten geändert wird oder Konten, Modelle oder API-Schlüssel automatisch abgerechnet werden.

## Konfiguration und Anmeldedaten

- `<config_dir>/native-plugins.json` speichert den Installationspfad, die Prüfsumme und den Status, nicht OAuth-Token.
- `<data_dir>/connections/accounts/NAME` speichert die Anmeldeinformationen des Plugins; WASM-Plugins für den Arbeitsbereich erhalten diese nicht.
- Das Menü zur Anmeldekonfiguration füllt die Modellkonfiguration aus. Sie müssen `chatgpt.json` nicht manuell erstellen.
- macOS und Linux verwenden private Verzeichnisse und Dateiberechtigungen. Windows erbt die ACL (Access Control List) des Benutzerverzeichnisses.
- Die Anwendung verschlüsselt keine Anmeldedaten und speichert diese nicht in einem System-Schlüssel. Böswillige native Programme, die unter demselben Benutzerkonto ausgeführt werden, können sie dennoch lesen.

Legen Sie diese Verzeichnisse nicht in Arbeitsbereiche, freigegebene Ordner, Versionskontrollsysteme oder öffentliche Backups. Direkte API-Schlüssel-Verbindungen funktionieren weiterhin ohne ein Anmelde-Plugin.

## Mit einem Browser koppeln

Nach der Installation und Aktivierung des Plugins, führen Sie folgenden Befehl auf demselben Computer wie den Browser aus:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Verwenden Sie für Claude im obigen Befehl `claude` statt `chatgpt`. Behalten Sie das Terminal aktiv und öffnen Sie den ausgedruckten Pairing-Link. Bestätigen Sie den Plugin, das Konto und das Modell auf der Seite, und speichern Sie dann. Das Pairing erstellt zunächst einen unsicheren Entwurf und ersetzt nicht automatisch eine bestehende Verbindung. Verwenden Sie nach dem Neustart des Dienstes einen neuen Link. Teilen Sie keine Pairing-Links: Jeder enthält ein lokales Capability-Geheim.

Der Dienst lauscht nur auf `127.0.0.1`, ist an die angegebene Seitenquelle gebunden und leitet keine beliebigen URLs weiter. HTTPS-Seiten, die auf die lokale Netzwerkverbindung zugreifen, benötigen möglicherweise die Erlaubnis für das lokale Netzwerk. Das Koppeln umgeht keine Browser-Berechtigungen oder CORS. Ein Telefon kann das native Plugin nicht eigenständig ausführen, und das Löschen einer Browser-Verbindung führt nicht zur Abmeldung vom nativen Konto.

## Einschränkungen und Fehlerbehebung bei ChatGPT

Das Plugin verwendet die offizielle OAuth-Dynamik für die Client-Registrierung, PKCE, eine separate Host-Identität und eine separate Speicherung von Anmeldeinformationen. Es liest Ihre persönlichen Dateien `~/.codex/auth.json` nicht und verwendet keine proprietären Client-IDs. Sehen Sie sich die [offizielle Dokumentation zur Anmeldung](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) und die Informationen zu [Profilen und Sitzungen](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions) an.

Inference verwendet die öffentliche Responses API, nicht die private `backend-api`. OpenAI legt die Voraussetzungen, die Richtlinien für den Arbeitsbereich und die Zugriffsbeschränkungen fest. Die Anmeldung garantiert keinen Zugriff auf Inference. Die autorisierung auf Abonnementbasis verwendet feste Einstellungen von `store: false` und `stream: true`. Die aktuelle Vorschau berücksichtigt die maximale Anzahl von Ausgabe-Token nicht; siehe [Einschränkungen der Vorschau](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Fehler treten bei der Austausch von Autorisierungs-Codes, dem Aktualisieren von Sitzungen und Netzwerkfehlern auf. Die Diagnose zeigt HTTP-Statuscodes, ohne die rohen Antworten des Token-Endpunkts, Anmeldeinformationen oder Autorisierungs-Codes anzuzeigen. Fehlgeschlagene Autorisierungen oder Modellanfragen werden nicht automatisch erneut versucht. Wenn ein Codeaustausch abgelehnt wird, starten Sie den Anmeldevorgang erneut und überprüfen Sie die Version, die Systemuhr, die Netzwerkeinstellungen und die Proxy-Einstellungen. Der Statuscode allein erklärt nicht, warum er abgelehnt wurde.

## Einschränkungen von Claude Console

Befolgen Sie die Anweisungen unter [official installation guide](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart), um `ant` zu installieren und sicherzustellen, dass es im PATH-Verzeichnis vorhanden ist. Das Verbindungspaket enthält weder `ant` noch führt es dessen Installationsskript für Sie aus. Verwenden Sie dann `i login claude`.

Das Plugin verwendet ein isoliertes HOME, `ANTHROPIC_CONFIG_DIR`, und sein eigenes Profil. Es liest keine vorhandenen Claude Code- oder persönlichen Ant-Profile. Es unterstützt derzeit nur ein Konsolenprofil. Eine erneute Autorisierung kann den Arbeitsbereich ändern, und nachfolgende Anfragen können den neuen Arbeitsbereich verwenden; starten Sie bei Bedarf eine neue Sitzung. **Die Nutzung von Claude wird über die Console API abgerechnet und nicht auf das Kontingent Ihres Pro- oder Max-Abos angerechnet.**

## Umfang der Überprüfung

Automatisierte Tests verwenden isolierte Verzeichnisse und lokale OAuth/TLS-Konfigurationen. Sie verwenden keine echten persönlichen Tokens, erteilen Ihnen keine Zugriffsrechte oder senden Abrechnungsanfragen. Das erfolgreiche Durchführen von Offline-Tests beweist nicht, dass ein Anbieter Zugriff auf ein echtes Konto gewährt. Die Hilfsfunktionen behalten ihre internen Protokolle für Worker und Dienste; Operationen, die für den Benutzer sichtbar sind, erfolgen über `i`.
