# Gerenciar plug-ins de login com i

> **O uso do Claude é cobrado pela API do Console e não consome a franquia da assinatura Pro ou Max.**
> Entrar sem informar uma chave de API não torna as solicitações gratuitas. Antes de autorizar o acesso, confira a organização, o espaço de trabalho e as permissões de cobrança no Console.

Use o `i` para gerenciar as conexões sem precisar memorizar os nomes dos executáveis auxiliares ou escrever as configurações de conexão manualmente. Os plugins de login nativos funcionam com as permissões do usuário do seu sistema operacional. Eles não são plugins de ferramentas WASM para o workspace; use o pacote de instalação correto.

## Configurar pelo terminal

Inicie o `i` e insira o comando de gerenciamento local, mesmo que você ainda não tenha configurado um modelo:

```text
/plugins
```

Escolha o ChatGPT ou o Claude, revise as permissões, e então instale e habilite o plugin. Digite `/login`, selecione um plugin de login e autorize-o no seu navegador do sistema. Escolha um modelo da lista ou insira um ID de modelo que você possa usar. Ao sucesso, o `i` salva a conexão para sua próxima tarefa. Você também pode inserir os comandos diretamente:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Use `/cancel` para fechar o menu. Você não pode alterar plugins ou conexões enquanto uma tarefa estiver em execução. As solicitações de autorização e listagem de modelos não enviam solicitações de geração. A alteração da conexão só ocorre após o sucesso tanto do login quanto da seleção do modelo. Falhas e cancelamentos mantêm a conexão anterior intacta, e o `i` salva uma cópia de backup da configuração privada antes de alternar.

## Gerenciar plug-ins pela linha de comando

### Alterar o modelo e o esforço de raciocínio

`/model` lê o catálogo de modelos do plugin de conexão nativo atual. Escolha um número da lista ou insira um ID de modelo diretamente. As conexões de API padrão não consultam um catálogo; insira o ID do modelo você mesmo. Isso não faz login novamente ou troca de contas:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Altere as configurações somente quando não houver tarefas, aprovações ou itens em espera. As configurações são salvas por conexão, com um backup privado antes de serem gravadas. Cancelar a lista de modelos mantém a configuração inalterada. Mudar apenas o modelo redefine o esforço para o padrão do provedor, para que você não transfira as configurações de outro modelo. `auto` e `default` deixam o esforço sem especificação. Outros valores são `none`, `minimal`, `low`, `medium`, `high`, `xhigh` e `max`. Nem todos os modelos suportam todos os valores, e o catálogo pode não anunciar suas capacidades. Alterar a configuração não envia uma solicitação de geração cobrável. O provedor valida o suporte em solicitações subsequentes. A API Responses usa `reasoning.effort`; a API Chat Completions usa `reasoning_effort`. O Claude usa `output_config.effort` com pensamento adaptativo. `none` solicita que o pensamento seja desativado nos modelos que suportam esse modo. Não assuma que todos os modelos suportam um esforço maior ou a desativação do pensamento. Consulte [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) e [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort).

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

Mude para uma conta existente usando `i plugins use chatgpt ACCOUNT_ID`. Use `i login chatgpt --account ACCOUNT_ID` para reautenticar uma inscrição existente, ou `i plugins logout chatgpt --account ACCOUNT_ID` para sair de uma conta específica. A reautenticação processa a inscrição sem copiar os tokens antigos. Não altera a conta na conexão atual até que você confirme o modelo.

Para gerenciar o plug-in do Claude Console, use `claude` no lugar de `chatgpt`. A instalação e as atualizações exigem confirmação. Após atualizar, reative o plugin e faça login para evitar substituir silenciosamente uma conexão ativa. Se você atualizar apenas o assistente e quiser manter a mesma conta, reative o plugin e execute `i plugins reconnect chatgpt`. Isso mantém a conta e as configurações do modelo atuais e vincula a conexão ativa ao novo executável verificado. Não faz login novamente nem envia uma solicitação de geração. Durante a instalação, o `i` mostra a verificação da versão, os bytes baixados, a verificação do checksum e o progresso do salvamento. Use `/cancel` para interromper. A conexão expira após 10 segundos e cada download pode levar até 120 segundos. Falhas não acionam uma tentativa automática ou habilitam o plugin. As descargas do GitHub usam as variáveis de ambiente do proxy do terminal, e não as configurações de proxy do seu navegador. Se necessário, comece com `HTTPS_PROXY=http://127.0.0.1:3067 i`, substituindo o endereço e a porta pelas suas próprias configurações de proxy.

Você também pode alterar as regras de proxy a partir do próprio `i`. Por padrão, as regras são salvas para cada usuário em `~/.i/config/proxy.json` (sob o diretório `I_CONFIG_DIR` se definido), e não no espaço de trabalho.

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

As regras se aplicam na seguinte ordem: regra específica para o propósito, configuração padrão do usuário, e, em seguida, o ambiente ou configuração original. Um `no` nunca retorna a um proxy de ambiente. As alterações afetam apenas as operações subsequentes; elas não interrompem ou reproduzem solicitações ativas. Configure os navegadores e serviços independentes em execução separadamente. HTTP, HTTPS, SOCKS5 e SOCKS5h são suportados. As chamadas de retorno de autorização local não utilizam o proxy. As URLs do proxy não podem incluir nomes de usuário, senhas, consultas ou caminhos, portanto, as credenciais não aparecem no histórico de comandos. Esta é uma ordem de comando do host; ela não expõe as configurações do proxy ou as credenciais da conta para os plugins WASM do workspace. Remover a seleção de instalação mantém os pacotes e credenciais antigos e não revoga a autorização remota. Para fazer logout, execute `i plugins logout NAME` antes de desativar ou remover o plugin. Se a revogação remota não for confirmada, verifique as configurações da conta do provedor.

O instalador baixa o arquivo de conexão correspondente do repositório oficial do GitHub Releases. Ele verifica o SHA-256 fornecido pelo GitHub, a lista de arquivos ZIP permitidos e a plataforma de destino, e verifica o checksum executável antes de cada invocação. Se nenhuma plataforma corresponder, ele para, em vez de executar scripts de instalação de terceiros. O checksum verifica a integridade, não a identidade do fornecedor. Os pacotes de conexão para macOS não são assinados ou notificados com Developer ID. A instalação do plugin não altera o PATH nem requer privilégios de administrador.

Os desenvolvedores podem testar um pacote construído localmente usando `i plugins install chatgpt --package /absolute/path/package.zip`. Após a confirmação, o instalador ainda verifica os membros do arquivo e a plataforma de destino. O plugin é iniciado desativado, e sua assinatura executável é verificada antes do uso. Um pacote local é rotulado como `local-build`, e não é verificado como uma versão oficial. Instale apenas o código nativo em que confia. Uma confirmação de retorno do navegador não significa que o login foi concluído. Aguarde a finalização da troca de tokens, verificação de identidade e seleção do modelo pelo terminal.

Arquivos `desktop.json` de nível de usuário mais antigos ainda podem listar `search` e `find_files`, que agora fazem parte do núcleo. Ao carregar, o host remove entradas de pesquisa antigas e incorporadas que são reconhecíveis e vincula os antigos trabalhadores incorporados do mesmo diretório de configuração do usuário ao executável atual, sem alterar o arquivo no disco. Nomes conflitantes para trabalhadores e plugins personalizados ainda geram um erro; o host não ignora as verificações de duplicidade. Esses erros ocorrem antes das solicitações de modelo, portanto, fazer login novamente é desnecessário.

### Diagnosticar falhas de geração

As conexões nativas do ChatGPT utilizam a API de Respostas com `store:false` e `stream:true`, e não as "Chat Completions". O terminal prefere SSE, mas também aceita um objeto de Respostas completo `application/json` se tiver `object:response`, um ID válido, `status:completed` e uma saída válida. Sem um cabeçalho `Content-Type`, ele detecta SSE ou JSON do corpo e, em seguida, aplica a mesma validação. Ele rejeita explicitamente HTML ou outros tipos de mídia não suportados. A análise de falhas mostra apenas a categoria de formato e o número de bytes, e não o corpo da resposta. HTML, objetos de "Chat Completions", JSON inválido e chamadas de ferramentas incompletas não são aceitos. Uma mudança no formato da resposta não causa o reenvio da solicitação. O serviço web local encaminha SSE válido em tempo real. Se o tipo de conteúdo estiver ausente ou a resposta for JSON, ele valida a resposta completa antes de fornecer SSE válido ao navegador. Este caminho de compatibilidade aguarda a conclusão e não reenvia automaticamente. Um erro HTTP 400 local no navegador não significa necessariamente um erro upstream: a análise de falhas distingue erros HTTP upstream, falhas de rede e falhas de formato.

Em caso de falha, o diagnóstico identifica a etapa: preparação da solicitação, autorização, HTTP, formato de resposta ou análise do fluxo. Os diagnósticos HTTP incluem apenas o status, códigos de erro conhecidos ou nomes de parâmetros, e um ID de solicitação limitado. Eles não exibem erros, mensagens ou tokens brutos. Erros de rede ou de formato não cancelam seu login ou não alteram automaticamente contas, modelos ou rotas de cobrança de chaves de API.

## Configuração e credenciais

- `<config_dir>/native-plugins.json` armazena o caminho de instalação, o hash e o estado de ativação, e não os tokens OAuth.
- `<data_dir>/connections/accounts/NAME` armazena as credenciais do plugin; os plugins WASM do workspace não as recebem.
- O menu de login preenche a configuração do modelo. Você não precisa escrever o arquivo `chatgpt.json`.
- macOS e Linux utilizam diretórios e permissões de arquivo privados. O Windows herda as permissões de acesso (ACL) do diretório do usuário.
- O aplicativo não criptografa as credenciais nem as armazena em um "chaveiro" do sistema. Programas nativos maliciosos que rodam sob o mesmo usuário ainda podem lê-las.

Não coloque essas pastas em espaços de trabalho, pastas compartilhadas, controle de versão ou backups públicos. As conexões diretas com a API continuam a funcionar sem a necessidade de um plugin de login.

## Parear com um navegador

Após instalar e ativar o plugin, execute o seguinte no mesmo computador em que o navegador está instalado:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Para o Claude, use `claude` no lugar de `chatgpt` no comando acima. Mantenha o terminal em execução e abra o link de pareamento impresso. Confirme o plugin, a conta e o modelo na página, e depois salve. O pareamento cria um rascunho não salvo inicialmente e não substitui automaticamente uma conexão existente. Use um novo link após reiniciar o serviço. Não compartilhe os links de pareamento: cada um contém um segredo de capacidade local.

O serviço só escuta em `127.0.0.1`, está vinculado à origem da página especificada e não proxy URLs arbitrários. Páginas HTTPS que acessam a rede local podem precisar de permissão de rede local. O emparelhamento não contorna as permissões do navegador ou o CORS. Um telefone não pode executar o plugin nativo sozinho, e a exclusão de uma conexão do navegador não desloga a conta nativa.

## Limitações do ChatGPT e solução de problemas

O plugin utiliza o registro dinâmico de clientes OAuth oficial, PKCE, uma identidade de host separada e armazenamento de credenciais separado. Ele não lê seu arquivo pessoal `~/.codex/auth.json` ou utiliza IDs de cliente proprietários. Consulte a [documentação oficial de login](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) e as seções sobre [perfis e sessões](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

A Inference utiliza a API pública de Respostas, e não a API privada `backend-api`. A OpenAI determina a elegibilidade, as políticas do espaço de trabalho e os limites de autorização; o login não garante o acesso à Inference. A autorização baseada em assinatura utiliza configurações fixas de `store: false` e `stream: true`. A versão atual ignora a configuração do número máximo de tokens de saída; veja [limitações da versão prévia](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Falhas são identificadas no processo de troca de códigos de autorização, renovação de sessão e erros de rede. Os diagnósticos exibem códigos de status HTTP, sem exibir as respostas do endpoint do token, credenciais ou códigos de autorização. Solicitações de autorização ou de modelo falhadas não são automaticamente repetidas. Se a troca de código for rejeitada, tente o login novamente e verifique a versão, o relógio do sistema, as configurações de rede e do proxy. O código de status sozinho não informa o motivo da rejeição.

## Limitações do Claude Console

Siga o [guia de instalação oficial](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) para instalar o `ant` e garantir que ele esteja no PATH. O pacote de conexão não inclui o `ant` nem executa o script de instalação para você. Em seguida, use o comando `i login claude`.

O plugin utiliza um ambiente HOME isolado, `ANTHROPIC_CONFIG_DIR`, e seu próprio perfil. Ele não lê códigos Claude existentes ou perfis de ant pessoais. Atualmente, suporta um único perfil de Console. A reautenticação pode alterar o espaço de trabalho, e solicitações subsequentes podem usar o novo espaço de trabalho; inicie uma nova sessão, se necessário. **O uso do Claude é cobrado pela API do Console e não consome a franquia da assinatura Pro ou Max.**

## Escopo da verificação

Os testes automatizados utilizam diretórios isolados e configurações locais de OAuth/TLS. Eles não utilizam tokens pessoais reais, não autorizam o acesso em seu nome nem enviam solicitações pagas. A passagem de testes offline não comprova que um provedor concederá acesso a uma conta real. Os auxiliares mantêm seus protocolos internos de trabalho e serviços; as operações visíveis para o usuário passam por `i`.
