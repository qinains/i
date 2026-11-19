# Aanmeldplug-ins beheren met i

> **Claude wordt gefactureerd via de Console API en valt niet onder de gebruikslimiet van je Pro- of Max-abonnement.**
> Aanmelden zonder een API-sleutel in te voeren maakt aanvragen niet gratis. Controleer in Console je organisatie, werkruimte en factureringsrechten voordat je toegang verleent.

Gebruik `i` om verbindingen te beheren zonder de namen van de bijbehorende uitvoerbare bestanden te onthouden of verbinding instellingen handmatig in te voeren. De native sign-in plugins werken met de gebruikersrechten van uw besturingssysteem. Ze zijn geen WASM-tools voor de workspace; gebruik het juiste installatiepakket.

## Instellen vanuit de terminal

Start `i` en voer het lokale beheercommando uit, zelfs als u nog geen model heeft geconfigureerd:

```text
/plugins
```

Kies ChatGPT of Claude, bekijk de instellingen, installeer en activeer vervolgens de plugin. Typ `/login`, selecteer een inlogplugin en autoriseer deze in uw systeembrowser. Kies een model uit de lijst of voer een model-ID in dat u kunt gebruiken. Bij succes, slaat `i` de verbinding op voor uw volgende taak. U kunt ook de commando's direct invoeren:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Gebruik `/cancel` om het menu te sluiten. U kunt geen plugins of verbindingen wijzigen terwijl een taak in uitvoering is. Het autoriseren en het weergeven van modellen sturen geen generatieverzoeken. De verbinding verandert pas nadat zowel de inlog en de selectie van het model succesvol zijn. Fouten en annuleringen laten de vorige verbinding intact, en `i` slaat een privéconfiguratie-back-up op voordat de overgang plaatsvindt.

## Plug-ins beheren via de opdrachtregel

### Het model en de redeneerinspanning wijzigen

`/model` leest de modelcatalogus uit het huidige native connectie-plugin. Kies een nummer uit de lijst of voer een model-ID direct in. Standaard API-key connecties vragen geen catalogus op; voer het model-ID zelf in. Dit betekent niet dat u opnieuw inlogt of accounts wisselt:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Pas de instellingen alleen aan wanneer er geen openstaande taken, goedkeuringen of items in de wachtlijst zijn. De instellingen worden per verbinding opgeslagen, met een privéback-up voordat ze worden opgeslagen. Het annuleren van de lijst met modellen, laat de configuratie ongewijzigd. Het alleen wijzigen van het model, reset de inspanning terug naar de standaardinstelling van de provider, zodat u de instellingen van een ander model niet overneemt. `auto` en `default` laten de inspanning ongedefinieerd. Andere waarden zijn `none`, `minimal`, `low`, `medium`, `high`, `xhigh` en `max`. Niet alle modellen ondersteunen elke waarde, en de catalogus geeft mogelijk niet alle mogelijkheden aan. Het wijzigen van de instelling verzendt geen vergoeding voor een generatieverzoek. De provider valideert de ondersteuning bij latere verzoeken. De Responses API gebruikt `reasoning.effort`; de Chat Completions API gebruikt `reasoning_effort`. Claude gebruikt `output_config.effort` met adaptieve denking. `none` vraagt om het uitschakelen van de denking op modellen die dit ondersteunen. Ga er niet van uit dat elk model een hogere inspanning of het uitschakelen van de denking ondersteunt. Zie [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) en [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort).

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

Koppel aan een bestaand account met `i plugins use chatgpt ACCOUNT_ID`. Gebruik `i login chatgpt --account ACCOUNT_ID` om een bestaande registratie opnieuw te autoriseren, of `i plugins logout chatgpt --account ACCOUNT_ID` om uit een specifiek account te loggen. De herautorisatie stelt de registratie in zonder de oude, wisselende tokens te kopiëren. Het verandert het account niet op de huidige verbinding totdat u het model bevestigt.

Gebruik voor de Claude Console-plug-in `claude` in plaats van `chatgpt`. De installatie en updates vereisen bevestiging. Na het updaten, activeer de plugin opnieuw en log in om te voorkomen dat een actieve verbinding stilzwijgend wordt vervangen. Als u alleen de helper update en de account hetzelfde wilt houden, activeer dan de plugin opnieuw en voer `i plugins reconnect chatgpt` uit. Dit behoudt het huidige account en modelinstellingen en verbindt de actieve verbinding met de geverifieerde nieuwe executable. Het logt niet opnieuw in of verzendt een generatieverzoek. Tijdens de installatie toont `i` informatie over de versie, gedownloade bytes, checksum-verificatie en het opslaan van de voortgang. Gebruik `/cancel` om te stoppen. De verbinding wordt na 10 seconden beëindigd, en elke download kan tot 120 seconden duren. Fouten veroorzaken geen automatische herstart of het activeren van de plugin. Downloads van GitHub gebruiken de proxy-omgevingsvariabelen van de terminal, niet de proxy-instellingen van uw browser. Indien nodig, start dan met `HTTPS_PROXY=http://127.0.0.1:3067 i`, waarbij u het adres en de poort vervangt door uw eigen proxy-instellingen.

U kunt ook proxy-regels wijzigen vanuit `i`. Standaard worden de regels per gebruiker opgeslagen in `~/.i/config/proxy.json` (onder `I_CONFIG_DIR` indien ingesteld), niet in de werkruimte.

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

De regels gelden in de volgende volgorde: regel per doel, standaardinstellingen van de gebruiker, en vervolgens de oorspronkelijke omgeving of configuratie. Een expliciete `no` keert nooit terug naar een omgevingspool. Wijzigingen hebben alleen invloed op latere operaties; ze onderbreken of herhalen geen actieve verzoeken. Configureer browsers en reeds lopende standalone-services apart. HTTP, HTTPS, SOCKS5 en SOCKS5h worden ondersteund. Lokale autorisatie-callback-functies gebruiken de pool niet. Pool-URLs mogen geen gebruikersnamen, wachtwoorden, queries of paden bevatten, zodat inloggegevens niet in de opdrachtgeschiedenis terechtkomen. Dit is een ingebouwde host-commando; het onthult geen poolinstellingen of accountgegevens voor WASM-plugins in de werkruimte. Het verwijderen van een installatiekeuze behoudt oude pakketten en inloggegevens en annuleert geen remote-autorisatie. Om uit te loggen, voer `i plugins logout NAME` uit voordat u de plugin uitschakelt of verwijdert. Als de remote-annulering niet bevestigd is, controleer dan de accountinstellingen van de provider.

De installatieprocedure downloadt de bijbehorende verbindingsbestand van de officiële GitHub Release. Het controleert de SHA-256 die door GitHub is verstrekt, de lijst met toegestane ZIP-bestanden en het beoogde platform, en controleert vervolgens de uitvoerbare bestand voordat elke keer dat het wordt gebruikt. Als er geen overeenkomend platform wordt gevonden, stopt het dan, in plaats van derden-partij installatiescripts uit te voeren. De checksum controleert de integriteit, niet de identiteit van de uitgever. De macOS **verbindingspakketten** zijn niet voorzien van een Developer ID-signatuur of notarization. De installatie van een plugin verandert de PATH niet en vereist geen administratorrechten.

Ontwikkelaars kunnen een lokaal gebouwd pakket testen met `i plugins install chatgpt --package /absolute/path/package.zip`. Na bevestiging controleert de installatie nog steeds de archiefbestanden en het beoogde platform. Het plugin wordt standaard uitgeschakeld, en de uitvoerbare checksum wordt gecontroleerd voordat het wordt gebruikt. Een lokaal pakket wordt gelabeld als `local-build` en wordt niet als officiële release beschouwd. Installeer alleen code waarvan u zeker bent. Een browser-callback bevestiging betekent niet dat de inlogprocedure voltooid is. Wacht tot de terminal de token-uitwisseling, identiteitsverificatie en modelselectie heeft afgerond.

Oudere bestanden met de naam `desktop.json` op gebruikersniveau kunnen nog steeds `search` en `find_files` bevatten, die nu zijn geïntegreerd in de kern. Bij het laden verwijdert de host bekende oude, ingebouwde zoekfunctionaliteiten en koppelt oude, ingebouwde processen uit hetzelfde gebruikersconfiguratiepad aan de huidige uitvoerbare, zonder het bestand op de schijf te wijzigen. Conflicterende namen voor aangepaste processen en plugins veroorzaken nog steeds een fout; de host voert geen dubbele controles uit. Deze fouten ontstaan vóór modelverzoeken, dus het opnieuw inloggen is niet nodig.

### Problemen met genereren diagnosticeren

De native ChatGPT-verbindingen gebruiken de Responses API met `store:false` en `stream:true`, niet Chat Completions. De terminal geeft de voorkeur aan SSE, maar accepteert ook een compleet `application/json` Responses-object als het `object:response`, een geldige ID, `status:completed` en geldige output heeft. Zonder een `Content-Type`-header detecteert het SSE of JSON uit het body en past vervolgens dezelfde validatie toe. Het verwijst expliciet gemarkeerde HTML of andere niet-ondersteunde mediatippen. De foutdiagnostiek toont alleen de categorie en het aantal bytes, niet het response-body. HTML, Chat Completions-objecten, ongeldig JSON en onvolledige tool-aanroepen worden niet geaccepteerd. Een wijziging in het response-formaat veroorzaakt niet dat een verzoek opnieuw wordt verzonden. De lokale web-service stuurt een geldige SSE door. Als het mediatiptype ontbreekt of de response JSON is, valideert het de volledige response voordat het een geldige SSE naar de browser stuurt. Deze compatibiliteitsroute wacht op voltooiing en stuurt automatisch niets opnieuw. Een lokale HTTP 400 in de browser betekent niet noodzakelijkerwijs dat de status upstream is: de diagnostiek onderscheidt upstream HTTP-fouten, netwerkproblemen en formatfouten.

In geval van een fout, identificeert de diagnose de fase: voorbereiding van de aanvraag, autorisatie, HTTP, responseformaat, of stream-parsing. HTTP-diagnoses bevatten alleen de status, bekende foutcodes of parameternamen, en een beperkte request-ID. Ze tonen geen ruwe fouten, prompts of tokens. Netwerk- of formaatfouten zorgen er niet voor dat uw inlog wordt opgelicht of automatisch worden overgeschakeld naar andere accounts, modellen of API-sleutelabonnementen.

## Configuratie en inloggegevens

- `<config_dir>/native-plugins.json` bevat de installatiemodus, de hash en de status (ingeschakeld/uitgeschakeld), niet OAuth-tokens.
- `<data_dir>/connections/accounts/NAME` bevat de inloggegevens van het plugin; WASM-plugins voor de workspace ontvangen deze niet.
- Het menu voor het inloggen vult de configuratie van het model in. U hoeft niet zelf `chatgpt.json` te maken.
- macOS en Linux gebruiken privé mappen en bestandstoestemmingen. Windows erft de ACL (Access Control List) van de gebruikersmap.
- De applicatie versleutelt geen inloggegevens en slaat deze niet op in een systeem sleutel. Schadelijke native programma's die als dezelfde gebruiker draaien, kunnen ze mogelijk nog steeds lezen.

Plaats deze mappen niet in werkruimtes, gedeelde mappen, versiebeheer of publieke back-ups. Directe API-sleutelverbindingen blijven werken zonder een inlogplugin.

## Koppelen met een browser

Na het installeren en activeren van de plugin, voer dit uit op dezelfde computer als de browser:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Gebruik voor Claude in de bovenstaande opdracht `claude` in plaats van `chatgpt`. Houd de terminal open en open de afgedrukte koppeling. Controleer de plugin, het account en het model op de pagina, en sla het op. Het koppelen creëert eerst een ongeslagen ontwerp en vervangt automatisch geen bestaande verbinding. Gebruik een nieuwe koppeling na het herstarten van de service. Deel geen koppeling links: elk bevat een lokaal geheime sleutel.

De service luistert alleen op `127.0.0.1`, is gekoppeld aan de opgegeven oorspronkelijke pagina, en stuurt geen willekeurige URL's door. HTTPS-pagina's die toegang hebben tot de loopback-verbinding, kunnen lokale netwerktoestemming vereisen. Het koppelen omzeilt geen browserrechten of CORS. Een telefoon kan het native plugin niet zelf uitvoeren, en het verwijderen van een browserverbinding zorgt niet voor het afsluiten van het native account.

## Beperkingen van ChatGPT en probleemoplossing

De plugin maakt gebruik van officiële OAuth-clientregistratie, PKCE, een aparte host-identiteit en een aparte opslag van credentials. Het leest uw persoonlijke `~/.codex/auth.json` bestand niet en gebruikt geen eigen client-ID's. Raadpleeg de [officiële documentatie voor inloggen](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) en de informatie over [profielen en sessies](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

Inference gebruikt de publieke Responses API, niet de private `backend-api`. OpenAI bepaalt de toegang, de beleidsregels voor de werkruimte en de limieten voor autorisatie; het inloggen garandeert geen toegang tot Inference. Autorisatie op basis van een abonnement gebruikt vaste instellingen, namelijk `store: false` en `stream: true`. De huidige preview negeert de instelling voor het maximale aantal tokens; zie [beperkingen van de preview](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Fouten worden onderscheiden in de uitwisseling van autorisatiecodes, het vernieuwen van sessies en netwerkfouten. De diagnose toont HTTP-statuscodes, zonder de ruwe antwoorden van de token-endpoint, inloggegevens of autorisatiecodes weer te geven. Mislukte autorisaties of modelverzoeken worden niet automatisch opnieuw geprobeerd. Als een code-uitwisseling wordt afgewezen, start u het inlogproces opnieuw en controleer dan de versie, de systeemklok, het netwerk en de instellingen voor de proxy. De statuscode alleen geeft niet aan waarom deze is afgewezen.

## Beperkingen van Claude Console

Volg de instructies in [official installation guide](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) om `ant` te installeren en ervoor te zorgen dat het in de PATH staat. Het verbindingspakket bevat `ant` niet en voert ook geen installatiescript voor u uit. Gebruik vervolgens `i login claude`.

De plugin gebruikt een geïsoleerde HOME, `ANTHROPIC_CONFIG_DIR`, en zijn eigen profiel. Het leest geen bestaande Claude Code of persoonlijke ant-profielen. Het ondersteunt momenteel één Console-profiel. Een nieuwe autorisatie kan de werkruimte wijzigen, en daaropvolgende verzoeken kunnen de nieuwe werkruimte gebruiken; start een nieuwe sessie indien nodig. **Claude wordt gefactureerd via de Console API en valt niet onder de gebruikslimiet van je Pro- of Max-abonnement.**

## Reikwijdte van de verificatie

Geautomatiseerde tests maken gebruik van geïsoleerde directories en lokale OAuth/TLS-fixtures. Ze gebruiken geen echte persoonlijke tokens, autoriseren toegang namens u, of sturen facturabele verzoeken. Het succesvol uitvoeren van offline tests bewijst niet dat een provider toegang zal verlenen tot een echt account. De helper-functies behouden hun interne werker- en serviceprotocollen; operaties die direct door de gebruiker worden uitgevoerd, gaan via `i`.
