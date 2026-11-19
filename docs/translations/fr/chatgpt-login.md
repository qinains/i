# Gérer les extensions de connexion avec i

> **Claude est facturé via l’API Console, et non imputé à votre abonnement Pro ou Max.**
> Se connecter sans saisir de clé API ne rend pas les requêtes gratuites. Vérifiez votre organisation, votre espace de travail et vos droits de facturation dans Console avant d’autoriser l’accès.

Utilisez `i` pour gérer les connexions sans avoir à mémoriser les noms des exécutables ou à saisir manuellement les paramètres de connexion. Les plugins d'authentification natifs fonctionnent avec les permissions de l'utilisateur de votre système d'exploitation. Ils ne sont pas des plugins d'outils WASM pour l'espace de travail ; utilisez le bon package d'installation.

## Configurer depuis le terminal

Démarrez `i` et entrez la commande de gestion locale, même si vous n'avez pas encore configuré un modèle :

```text
/plugins
```

Choisissez ChatGPT ou Claude, examinez les autorisations, puis installez et activez le plugin. Entrez `/login`, sélectionnez un plugin d'authentification et autorisez-le dans votre navigateur système. Choisissez un modèle dans la liste ou entrez un ID de modèle que vous pouvez utiliser. En cas de succès, `i` enregistre la connexion pour votre prochaine tâche. Vous pouvez également saisir les commandes directement :

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Utilisez `/cancel` pour fermer le menu. Vous ne pouvez pas modifier les plugins ou les connexions pendant que l'exécution d'une tâche est en cours. Les opérations d'autorisation et de liste de modèles ne génèrent pas de requêtes. Les modifications de connexion ne sont effectuées qu'après que les deux étapes (connexion et sélection du modèle) ont réussi. Les erreurs et les annulations laissent la connexion précédente intacte, et `i` sauvegarde une copie de configuration privée avant de basculer.

## Gérer les extensions en ligne de commande

### Changer de modèle et régler l’effort de raisonnement

`/model` lit le catalogue de modèles à partir du plugin de connexion natif actuel. Choisissez un numéro dans la liste ou entrez directement l'ID du modèle. Les connexions API standard ne consultent pas de catalogue ; entrez l'ID du modèle vous-même. Cela ne nécessite pas de nouvelle connexion ou de changement de compte :

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Modifiez les paramètres uniquement lorsque aucune tâche, approbation ou élément en attente n'est présente. Les paramètres sont sauvegardés par connexion, avec une sauvegarde privée avant l'écriture. Annuler la liste des modèles ne modifie pas la configuration. Basculer uniquement sur un modèle réinitialise l'effort à la valeur par défaut du fournisseur, de sorte que vous ne transmettiez pas les paramètres d'un autre modèle. `auto` et `default` laissent l'effort non spécifié. Les autres valeurs sont `none`, `minimal`, `low`, `medium`, `high`, `xhigh` et `max`. Tous les modèles ne prennent pas en charge toutes les valeurs, et le catalogue peut ne pas les annoncer. Modifier le paramètre ne déclenche pas une demande de génération facturable. Le fournisseur valide le support lors des demandes suivantes. L’API Responses utilise `reasoning.effort` ; l’API Chat Completions utilise `reasoning_effort`. Claude utilise `output_config.effort` avec une pensée adaptative. `none` demande que la pensée soit désactivée sur les modèles qui prennent en charge ce mode. Ne présumez pas que tous les modèles prennent en charge un effort plus important ou la désactivation de la pensée. Consultez [la raisonnement d'OpenAI](https://developers.openai.com/api/docs/guides/reasoning) et [l'effort de Claude](https://platform.claude.com/docs/en/build-with-claude/effort).

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

Passez à un compte existant en utilisant `i plugins use chatgpt ACCOUNT_ID`. Utilisez `i login chatgpt --account ACCOUNT_ID` pour réautoriser une inscription existante, ou `i plugins logout chatgpt --account ACCOUNT_ID` pour se déconnecter d'un compte spécifique. La réautorisation permet de valider l'inscription sans copier les anciens jetons. Elle ne modifie pas le compte sur la connexion actuelle tant que vous n'avez pas confirmé le modèle.

Pour gérer le plugin Claude Console, utilisez `claude` à la place de `chatgpt`. L'installation et les mises à jour nécessitent une confirmation. Après la mise à jour, réactivez le plugin et connectez-vous pour éviter de remplacer silencieusement une connexion active. Si vous mettez à jour uniquement l'assistant et souhaitez conserver le même compte, réactivez le plugin et exécutez la commande `i plugins reconnect chatgpt`. Cela conserve le compte et les paramètres du modèle actuels et lie la connexion active à l'exécutable nouvellement vérifié. Cela ne nécessite pas de nouvelle connexion ni d'envoi d'une requête de génération. Lors de l'installation, `i` affiche la recherche de la version, le nombre de bytes téléchargés, la vérification du checksum et la sauvegarde de la progression. Utilisez `/cancel` pour arrêter. La connexion expire après 10 secondes et chaque téléchargement peut prendre jusqu'à 120 secondes. Les erreurs ne déclenchent pas de nouvelle tentative automatique ni n'activent le plugin. Les téléchargements depuis GitHub utilisent les variables d'environnement du proxy de la terminal, et non les paramètres de proxy de votre navigateur. Si nécessaire, commencez avec `HTTPS_PROXY=http://127.0.0.1:3067 i`, en remplaçant l'adresse et le port par vos propres paramètres de proxy.

Vous pouvez également modifier les règles de proxy depuis l'interface `i`. Par défaut, les règles sont enregistrées pour chaque utilisateur dans `~/.i/config/proxy.json` (sous `I_CONFIG_DIR` si défini), et non dans l'espace de travail :

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

Les règles s'appliquent dans l'ordre suivant : règle spécifique à un objectif, paramètres par défaut de l'utilisateur, puis environnement ou configuration d'origine. Une configuration explicite `no` ne revient jamais à un proxy d'environnement. Les modifications affectent uniquement les opérations ultérieures ; elles n'interrompent ni ne réexécutent les requêtes actives. Configurez les navigateurs et les services autonomes déjà en cours d'exécution séparément. HTTP, HTTPS, SOCKS5 et SOCKS5h sont pris en charge. Les appels de rappel d'autorisation locaux n'utilisent pas le proxy. Les URL de proxy ne peuvent pas inclure de noms d'utilisateur, de mots de passe, de requêtes ou de chemins, de sorte que les informations d'identification ne se retrouvent pas dans l'historique des commandes. Il s'agit d'une commande hôte intégrée ; elle ne divulgue pas les paramètres du proxy ou les informations d'identification du compte aux plugins WASM de l'espace de travail. La suppression d'une sélection d'installation conserve les anciens paquets et les informations d'identification et ne revoke pas l'autorisation à distance. Pour vous déconnecter, exécutez `i plugins logout NAME` avant de désactiver ou de supprimer le plugin. Si la révocation à distance n'est pas confirmée, vérifiez les paramètres du compte auprès du fournisseur.

L'installateur télécharge l'attache de connexion correspondante à partir de la version officielle sur GitHub. Il vérifie le SHA-256 fourni par GitHub, la liste des membres ZIP autorisés et la plateforme cible, puis vérifie le hachage de l'exécutable avant chaque invocation. Si aucune plateforme ne correspond, il s'arrête plutôt que d'exécuter des scripts d'installation tiers. Le hachage vérifie l'intégrité, et non l'identité de l'éditeur. Les paquets de connexion macOS ne sont pas signés par Developer ID ni notifiés. L'installation du plugin ne modifie pas le PATH ni ne nécessite des privilèges d'administrateur.

Les développeurs peuvent tester un package créé localement avec `i plugins install chatgpt --package /absolute/path/package.zip`. Une fois la confirmation effectuée, l'installateur vérifie toujours les membres de l'archive et la plateforme cible. Le plugin démarre avec des paramètres désactivés, et son code exécutable est vérifié avant utilisation. Un package local est étiqueté `local-build`, et n'est pas considéré comme une version officielle. Installez uniquement le code natif que vous jugez fiable. Une confirmation de retour du navigateur ne signifie pas que la connexion est terminée. Attendez que le terminal ait terminé l'échange de jetons, la vérification de l'identité et le choix du modèle.

Les anciens fichiers `desktop.json` au niveau de l'utilisateur peuvent toujours mentionner `search` et `find_files`, qui sont désormais intégrés au noyau. Lors du chargement, le système supprime les anciennes entrées de recherche intégrées et associe les anciens workers intégrés du même répertoire de configuration utilisateur à l'exécutable actuel, sans modifier le fichier sur le disque. Les noms conflictuels pour les workers et les plugins personnalisés provoquent toujours une erreur ; le système ne réalise pas de vérifications de doublons. Ces erreurs se produisent avant les requêtes de modèle, de sorte que se reconnecter n'est pas nécessaire.

### Diagnostiquer un échec de génération

Les connexions natives ChatGPT utilisent l'API Responses avec `store:false` et `stream:true`, et non les "Chat Completions". Le terminal préfère SSE, mais accepte également un objet "Responses" complet au format `application/json` s'il contient `object:response`, un ID valide, `status:completed` et une sortie valide. Sans en-tête `Content-Type`, il détecte SSE ou JSON à partir du corps et applique ensuite la même validation. Il rejette explicitement les types HTML ou autres types de médias non pris en charge. Les diagnostics d'échec ne montrent que la catégorie du format et le nombre de bytes, et non le corps de la réponse. Les objets "Chat Completions", le JSON invalide et les appels d'outils incomplets ne sont pas acceptés. Un changement dans le format de réponse ne provoque pas le renvoi d'une requête. Le service web local transmet le SSE valide en direct. Si le type de contenu est manquant ou si la réponse est au format JSON, il valide la réponse complète avant de fournir le SSE valide au navigateur. Ce chemin de compatibilité attend la fin et ne renvoie pas automatiquement. Une erreur HTTP 400 locale dans le navigateur ne signifie pas nécessairement un problème sur le serveur : les diagnostics distinguent les erreurs HTTP, les pannes réseau et les erreurs de format.

En cas d'échec, le diagnostic identifie l'étape : préparation de la requête, autorisation, HTTP, format de réponse ou analyse du flux. Les diagnostics HTTP ne comprennent que le statut, les codes d'erreur connus ou les noms de paramètres, ainsi qu'un identifiant de requête limité. Ils ne présentent pas les erreurs brutes, les invites ou les jetons. Les erreurs de réseau ou de format ne permettent pas de réinitialiser votre connexion ou de basculer automatiquement entre les comptes, les modèles ou les chemins de facturation de clé API.

## Configuration et identifiants

- `<config_dir>/native-plugins.json` stocke le chemin d'installation, le hachage et l'état d'activation, et non les jetons OAuth.
- `<data_dir>/connections/accounts/NAME` stocke les informations d'identification du plugin ; les plugins WASM pour l'espace de travail ne les reçoivent pas.
- Le menu de connexion permet de configurer le modèle. Vous n'avez pas besoin de créer un fichier `chatgpt.json`.
- macOS et Linux utilisent des répertoires et des permissions de fichiers privés. Windows hérite des ACL (listes de contrôle d'accès) du répertoire de l'utilisateur.
- L'application ne chiffre pas les identifiants ni ne les stocke dans un coffre-fort système. Des programmes natifs malveillants exécutés sous le même compte utilisateur peuvent toujours les lire.

Ne placez pas ces répertoires dans des espaces de travail, des dossiers partagés, des systèmes de contrôle de version ou des sauvegardes publiques. Les connexions directes à l'API continuent de fonctionner sans le plugin d'authentification.

## Associer un navigateur

Après avoir installé et activé le plugin, exécutez la commande suivante sur le même ordinateur que votre navigateur :

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Pour Claude, utilisez `claude` à la place de `chatgpt` dans cette commande. Maintenez le terminal ouvert et ouvrez le lien de jumelage imprimé. Confirmez le plugin, le compte et le modèle sur la page, puis enregistrez. Le jumelage crée d'abord une version non enregistrée et ne remplace pas automatiquement une connexion existante. Utilisez un nouveau lien après avoir redémarré le service. Ne partagez pas les liens de jumelage : chacun contient un secret de capacité local.

Le service ne fonctionne que sur `127.0.0.1`, est lié à l'origine de la page spécifiée et ne promeut pas d'URL arbitraires. Les pages HTTPS accédant au réseau local peuvent nécessiter des autorisations de réseau local. La configuration ne contourne pas les permissions du navigateur ou les politiques CORS. Un téléphone ne peut pas exécuter le plugin nativement, et la suppression d'une connexion de navigateur ne se déconnecte pas du compte native.

## Limites de ChatGPT et dépannage

Le plugin utilise l'enregistrement dynamique de client OAuth officiel, PKCE, une identité d'hôte distincte et un stockage de jetons séparé. Il ne lit pas votre fichier `~/.codex/auth.json` personnel ni ne copie d'identifiants de client propriétaires. Consultez la [documentation officielle d'authentification](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) et les [profils et sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

Inference utilise l'API publique Responses, et non l'API privée `backend-api`. OpenAI détermine l'éligibilité, les règles du compte et les limites d'autorisation ; la connexion ne garantit pas l'accès à Inference. L'autorisation basée sur un abonnement utilise des paramètres fixes : `store: false` et `stream: true`. La version actuelle ignore la limite de nombre de tokens de sortie ; voir [limites de la version préliminaire](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Les erreurs concernent l'échange de codes d'autorisation, la mise à jour de la session et les erreurs réseau. Les diagnostics affichent les codes de statut HTTP, sans afficher les réponses des points d'extrémité des jetons, les informations d'identification ou les codes d'autorisation. Les demandes d'autorisation ou de modèles échouées ne sont pas automatiquement relancées. Si un échange de codes est rejeté, redémarrez la connexion et vérifiez la version, l'heure du système, le réseau et les paramètres du proxy. Le seul code de statut ne vous indique pas pourquoi il a été rejeté.

## Limites de Claude Console

Suivez le [guide d'installation officiel](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) pour installer `ant` et vous assurer qu'il est présent dans la variable d'environnement PATH. Le package de connexion ne contient ni `ant` ni ne lance son script d'installation pour vous. Ensuite, utilisez la commande `i login claude`.

Le plugin utilise un environnement HOME isolé, `ANTHROPIC_CONFIG_DIR`, et son propre profil. Il ne lit pas les anciens codes Claude ou les profils d'ant personnels. Il prend actuellement en charge un seul profil de console. La réautorisation peut modifier l'espace de travail, et les demandes suivantes peuvent utiliser le nouvel espace de travail ; démarrez une nouvelle session si nécessaire. **Claude est facturé via l’API Console, et non imputé à votre abonnement Pro ou Max.**

## Périmètre de la vérification

Les tests automatisés utilisent des répertoires isolés et des fixtures OAuth/TLS locales. Ils n'utilisent pas de jetons personnels réels, ne n'autorisent pas l'accès en votre nom, ni n'envoyent de requêtes facturables. Le succès des tests hors ligne ne prouve pas que un fournisseur accordera l'accès à un compte réel. Les fonctions auxiliaires conservent leurs protocoles internes pour les travailleurs et les services ; les opérations visibles par l'utilisateur passent par `i`.
