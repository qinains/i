# Gestire i plugin di accesso con i

> **Claude usa la fatturazione dell’API di Console, non la quota dell’abbonamento Pro o Max.**
> Accedere senza inserire una chiave API non rende gratuite le richieste. Prima di autorizzare l’accesso, controlla in Console l’organizzazione, lo spazio di lavoro e i permessi di fatturazione.

Utilizza `i` per gestire le connessioni senza dover ricordare i nomi degli eseguibili helper o scrivere manualmente le impostazioni di connessione. I plugin di autenticazione nativi funzionano con le stesse autorizzazioni dell'utente del sistema operativo. Non sono plugin per strumenti WASM di workspace; utilizzare il pacchetto di installazione corretto.

## Configurare dal terminale

Iniziare con `i` e inserire il comando di gestione locale, anche se non hai ancora configurato un modello:

```text
/plugins
```

Scegliete ChatGPT o Claude, rivedete le autorizzazioni, quindi installate e abilitate il plugin. Digitate `/login`, selezionate un plugin di accesso e autorizzatelo nel vostro browser di sistema. Scegliete un modello dall'elenco o inserite un ID di modello che potete utilizzare. In caso di successo, `i` salva la connessione per la vostra prossima attività. Potete anche inserire i comandi direttamente:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Utilizza `/cancel` per chiudere il menu. Non è possibile modificare i plugin o le connessioni mentre un'attività è in esecuzione. Le richieste di autorizzazione e l'elenco dei modelli non inviano richieste di generazione. Le modifiche alla connessione avvengono solo dopo che sia l'accesso che la selezione del modello sono andati a buon fine. In caso di errori o cancellazioni, la connessione precedente rimane intatta e `i` salva una copia di backup della configurazione privata prima di passare a una nuova.

## Gestire i plugin dalla riga di comando

### Cambiare il modello e il livello di ragionamento

`/model` legge il catalogo dei modelli dal plugin di connessione nativo corrente. Seleziona un numero dall'elenco o inserisci direttamente l'ID del modello. Le connessioni API standard non consultano un catalogo; devi inserire manualmente l'ID del modello. Questo non richiede un nuovo accesso o la modifica dell'account:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Modificare le impostazioni solo quando non ci sono attività, approvazioni o elementi in attesa. Le impostazioni vengono salvate per ogni connessione, con un backup privato prima della scrittura. Annullare l'elenco dei modelli lascia la configurazione invariata. Cambiare solo il modello riporta lo sforzo al valore predefinito del fornitore, in modo da non trasferire le impostazioni di un altro modello. `auto` e `default` lasciano lo sforzo non specificato. Altri valori sono `none`, `minimal`, `low`, `medium`, `high`, `xhigh` e `max`. Non tutti i modelli supportano tutti i valori, e il catalogo potrebbe non pubblicizzare le loro funzionalità. Modificare l'impostazione non invia una richiesta di generazione a pagamento. Il fornitore convalida il supporto nelle richieste successive. L’API Responses usa `reasoning.effort`, mentre l’API Chat Completions usa `reasoning_effort`. Claude utilizza `output_config.effort` con un approccio di pensiero adattivo. `none` richiede che il pensiero sia disabilitato nei modelli che supportano questa modalità. Non presumere che ogni modello supporti uno sforzo maggiore o la disabilitazione del pensiero. Consultare [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) e [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort).

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

Passa a un account esistente utilizzando `i plugins use chatgpt ACCOUNT_ID`. Utilizza `i login chatgpt --account ACCOUNT_ID` per riautorizzare una registrazione esistente, oppure `i plugins logout chatgpt --account ACCOUNT_ID` per disconnettersi da un account specifico. La riautorizzazione consente di registrare senza copiare i vecchi token. Non modifica l'account nella connessione corrente fino a quando non confermi il modello.

Per gestire il plugin Claude Console, usa `claude` al posto di `chatgpt`. L'installazione e gli aggiornamenti richiedono conferma. Dopo l'aggiornamento, riattivate il plugin e effettuate l'accesso per evitare di sostituire silenziosamente una connessione attiva. Se aggiornate solo l'helper e desiderate mantenere lo stesso account, riattivate il plugin ed eseguite il comando `i plugins reconnect chatgpt`. Questo mantiene l'account e le impostazioni del modello correnti e collega la connessione attiva all'eseguibile verificato. Non è necessario effettuare nuovamente l'accesso o inviare una richiesta di generazione. Durante l'installazione, `i` mostra la verifica della versione, i byte scaricati, la verifica del checksum e il salvataggio dei progressi. Utilizzate `/cancel` per interrompere. La connessione scade dopo 10 secondi e ogni download può richiedere fino a 120 secondi. I fallimenti non attivano un tentativo automatico o abilitano il plugin. I download da GitHub utilizzano le variabili di ambiente del proxy del terminale, non le impostazioni del proxy del browser. Se necessario, iniziate con `HTTPS_PROXY=http://127.0.0.1:3067 i`, sostituendo l'indirizzo e la porta con le vostre impostazioni del proxy.

È inoltre possibile modificare le regole del proxy direttamente all'interno di `i`. Per impostazione predefinita, le regole vengono salvate per utente in `~/.i/config/proxy.json` (sotto `I_CONFIG_DIR` se impostato), e non nell'ambiente di lavoro.

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

Le regole si applicano nell'ordine seguente: regola specifica per lo scopo, impostazioni predefinite dell'utente, quindi l'ambiente o la configurazione originale. Un'esplicita impostazione `no` non torna indietro a un proxy di ambiente. Le modifiche interessano solo le operazioni successive; non interrompono o ripetono le richieste attive. Configurare separatamente i browser e i servizi autonomi già in esecuzione. Sono supportati HTTP, HTTPS, SOCKS5 e SOCKS5h. Le chiamate di callback di autorizzazione locale non utilizzano il proxy. Gli URL del proxy non possono includere nomi utente, password, query o percorsi, quindi le credenziali non finiscono nella cronologia dei comandi. Si tratta di un comando host integrato; non espone le impostazioni del proxy o le credenziali dell'account ai plugin WASM dell'ambiente di lavoro. Eliminare la selezione dell'installazione conserva i vecchi pacchetti e le credenziali e non revoca l'autorizzazione remota. Per disconnettersi, eseguire `i plugins logout NAME` prima di disabilitare o rimuovere il plugin. Se la revoca remota non viene confermata, controllare le impostazioni dell'account del provider.

L'installatore scarica l'allegato di connessione corrispondente dall'ufficiale repository GitHub Releases. Verifica l'hash SHA-256 fornito da GitHub, l'elenco consentito dei membri ZIP e la piattaforma di destinazione, quindi verifica l'integrità dell'eseguibile prima di ogni esecuzione. Se non viene trovata alcuna piattaforma compatibile, l'installazione si interrompe piuttosto che eseguire script di installazione di terze parti. L'hash verifica l'integrità, non l'identità del fornitore. I pacchetti di connessione per macOS non sono firmati con Developer ID o notarizzati. L'installazione del plugin non modifica la variabile PATH né richiede privilegi di amministratore.

Gli sviluppatori possono testare un pacchetto creato localmente utilizzando `i plugins install chatgpt --package /absolute/path/package.zip`. Dopo la conferma, l'installer verifica ancora i membri dell'archivio e la piattaforma di destinazione. Il plugin viene avviato in stato disabilitato e viene controllata la sua firma esecutiva prima dell'uso. Un pacchetto locale è etichettato come `local-build`, ma non è stato verificato come rilascio ufficiale. Installare solo il codice nativo di cui ci si fida. Una conferma di callback del browser non significa che l'accesso è completo. Attendere che il terminale completi lo scambio di token, la verifica dell'identità e la selezione del modello.

I file `desktop.json` a livello utente più vecchi potrebbero ancora elencare `search` e `find_files`, che ora sono integrati nel core. All'avvio, il sistema elimina le vecchie voci di ricerca integrate riconoscibili e associa i vecchi worker integrati dallo stesso percorso di configurazione dell'utente all'eseguibile corrente, senza modificare il file sul disco. I conflitti di nomi per i worker e i plugin personalizzati continuano a generare un errore; il sistema non effettua controlli di duplicazione. Questi errori si verificano prima delle richieste di modello, quindi è inutile effettuare nuovamente l'accesso.

### Diagnosticare gli errori di generazione

Le connessioni native di ChatGPT utilizzano l'API Responses con `store:false` e `stream:true`, non le "Chat Completions". Il terminale preferisce SSE, ma accetta anche un oggetto Responses completo `application/json` se contiene `object:response`, un ID valido, `status:completed` e un output valido. Senza un header `Content-Type`, rileva SSE o JSON dal corpo e quindi applica la stessa validazione. Rifiuta esplicitamente HTML o altri tipi di media non supportati. La diagnostica dei fallimenti mostra solo la categoria del formato e il numero di byte, non il corpo della risposta. HTML, oggetti "Chat Completions", JSON non valido e chiamate di strumenti incomplete non sono accettati. Un cambiamento nel formato della risposta non causa la necessità di reinviare la richiesta. Il servizio web locale inoltra SSE valido in tempo reale. Se il tipo di contenuto manca o la risposta è in formato JSON, convalida l'intera risposta prima di fornire SSE valido al browser. Questo percorso di compatibilità attende il completamento e non reinvia automaticamente. Un errore HTTP 400 locale nel browser non indica necessariamente lo stato upstream: la diagnostica distingue gli errori HTTP upstream, i guasti di rete e i fallimenti di formato.

In caso di errore, la diagnostica identifica la fase: preparazione della richiesta, autorizzazione, formato HTTP, formato della risposta o analisi del flusso. Le diagnostiche HTTP includono solo lo stato, i codici di errore noti o i nomi dei parametri, e un ID di richiesta limitato. Non visualizzano errori, messaggi o token grezzi. Gli errori di rete o di formato non cancellano la tua autenticazione o non passano automaticamente tra account, modelli o percorsi di fatturazione API.

## Configurazione e credenziali

- `<config_dir>/native-plugins.json` memorizza il percorso di installazione, l'hash e lo stato di attivazione, ma non i token OAuth.
- `<data_dir>/connections/accounts/NAME` memorizza le credenziali del plugin; i plugin WASM per l'ambiente di lavoro non le ricevono.
- Il menu di accesso compila la configurazione del modello. Non è necessario scrivere `chatgpt.json`.
- macOS e Linux utilizzano directory e permessi di file privati. Windows eredita le ACL (Access Control Lists) della directory utente.
- L'applicazione non crittografa le credenziali né le memorizza in un portachiavi di sistema. Programmi nativi malevoli che vengono eseguiti con lo stesso utente potrebbero comunque leggerle.

Non inserire queste directory in ambienti di lavoro, cartelle condivise, sistemi di controllo delle versioni o backup pubblici. Le connessioni API-key continuano a funzionare senza la necessità di un plugin di autenticazione.

## Associare un browser

Dopo aver installato e abilitato il plugin, eseguite questo comando sullo stesso computer da cui utilizzate il browser:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Per Claude, usa `claude` al posto di `chatgpt` nel comando precedente. Mantenere il terminale attivo e aprire il link di accoppiamento stampato. Confermare il plugin, l'account e il modello nella pagina, quindi salvare. L'accoppiamento crea prima una bozza non salvata e non sostituisce automaticamente una connessione esistente. Utilizzare un nuovo link dopo aver riavviato il servizio. Non condividere i link di accoppiamento: ognuno contiene un segreto di capacità locale.

Il servizio ascolta solo su `127.0.0.1`, è associato all'origine della pagina specificata e non inoltra URL arbitrari. Le pagine HTTPS che accedono alla rete locale potrebbero richiedere il permesso di accesso alla rete locale. La sincronizzazione non aggira le autorizzazioni del browser o le politiche CORS. Un telefono non può eseguire il plugin nativo da solo, e la disconnessione da una connessione del browser non disconnette l'account nativo.

## Limitazioni di ChatGPT e risoluzione dei problemi

Il plugin utilizza la registrazione dinamica del client tramite OAuth ufficiale, PKCE, un'identità separata per l'host e un sistema di archiviazione dei credenziali separato. Non legge il tuo file `~/.codex/auth.json` personale né utilizza ID client proprietari. Consulta la [documentazione ufficiale per l'accesso](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) e le sezioni relative a [profili e sessioni](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

Inference utilizza l'API pubblica Responses, non l'API privata `backend-api`. OpenAI determina l'idoneità, le politiche dello spazio di lavoro e i limiti di autorizzazione; l'accesso tramite login non garantisce l'accesso a Inference. L'autorizzazione basata sull'abbonamento utilizza impostazioni fisse come `store: false` e `stream: true`. La versione attuale ignora l'impostazione del numero massimo di token di output; vedere [limitazioni della versione di anteprima](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

I fallimenti riguardano lo scambio di codici di autorizzazione, il refresh della sessione e gli errori di rete. Le diagnosi mostrano i codici di stato HTTP senza visualizzare le risposte dell'endpoint del token, le credenziali o i codici di autorizzazione. In caso di fallimento dell'autorizzazione o di richieste di modello, non vengono automaticamente ripetute. Se lo scambio di codici viene rifiutato, riavviare il processo di accesso e verificare la versione, l'orologio di sistema, le impostazioni di rete e del proxy. Il solo codice di stato non vi dice perché è stato rifiutato.

## Limitazioni di Claude Console

Seguire le istruzioni contenute in [official installation guide](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) per installare `ant` e assicurarsi che sia presente nella variabile d'ambiente PATH. Il pacchetto di connessione non include né `ant` né esegue automaticamente lo script di installazione per te. Quindi, utilizza `i login claude`.

Il plugin utilizza un ambiente HOME isolato, `ANTHROPIC_CONFIG_DIR`, e il proprio profilo. Non legge i file di codice esistenti di Claude o i profili personali. Attualmente supporta un solo profilo di Console. La riautorizzazione può modificare lo spazio di lavoro, e le richieste successive potrebbero utilizzare il nuovo spazio di lavoro; avviare una nuova sessione se necessario. **Claude usa la fatturazione dell’API di Console, non la quota dell’abbonamento Pro o Max.**

## Ambito della verifica

I test automatizzati utilizzano directory isolate e fixture OAuth/TLS locali. Non utilizzano token personali reali, non autorizzano l'accesso a tuo nome né inviano richieste a pagamento. Il superamento dei test offline non dimostra che un fornitore concederà l'accesso a un account reale. Gli helper mantengono i propri protocolli interni per i worker e i servizi; le operazioni visibili all'utente vengono gestite tramite `i`.
