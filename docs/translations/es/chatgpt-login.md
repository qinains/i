# Gestionar los complementos de inicio de sesión con i

> **Claude se factura a través de la API de Console, no con la cuota de tu suscripción Pro o Max.**
> Iniciar sesión sin introducir una clave de API no hace que las solicitudes sean gratuitas. Comprueba tu organización, el espacio de trabajo y los permisos de facturación de Console antes de autorizar el acceso.

Utilice `i` para gestionar las conexiones sin tener que recordar los nombres de los ejecutables auxiliares o escribir manualmente la configuración de conexión. Los plugins de inicio de sesión nativos funcionan con los permisos del usuario del sistema operativo. No son plugins de herramientas WASM para el entorno de trabajo; utilice el paquete de instalación correcto.

## Configurar desde el terminal

Inicie `i` y introduzca el comando de gestión local, incluso si aún no ha configurado un modelo:

```text
/plugins
```

Elige ChatGPT o Claude, revisa los permisos, luego instala y activa el plugin. Introduce `/login`, selecciona un plugin de inicio de sesión y autorízalo en tu navegador del sistema. Elige un modelo de la lista o introduce un ID de modelo que puedas utilizar. Una vez que se haya completado con éxito, `i` guarda la conexión para tu próxima tarea. También puedes introducir los comandos directamente:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Utilice `/cancel` para cerrar el menú. No puede modificar los complementos o las conexiones mientras una tarea esté en ejecución. Las solicitudes de autorización y listado de modelos no envían solicitudes de generación. Los cambios en la conexión solo se realizan después de que tanto el inicio de sesión como la selección del modelo tengan éxito. Los fallos y las cancelaciones mantienen la conexión anterior intacta, y `i` guarda una copia de seguridad de la configuración privada antes de cambiar.

## Gestionar los complementos desde la línea de comandos

### Cambiar el modelo y el esfuerzo de razonamiento

`/model` lee el catálogo de modelos desde el plugin de conexión nativo actual. Elija un número de la lista o introduzca un ID de modelo directamente. Las conexiones de API estándar no consultan un catálogo; debe introducir el ID del modelo usted mismo. Esto no implica iniciar sesión de nuevo ni cambiar de cuenta:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Modifique la configuración únicamente cuando no haya tareas, aprobaciones o elementos en cola pendientes. La configuración se guarda por conexión, con una copia de seguridad privada antes de guardarla. Cancelar la lista de modelos deja la configuración sin cambios. Cambiar solo el modelo restablece el esfuerzo a la configuración predeterminada del proveedor, por lo que no se transfieren las configuraciones de otro modelo. `auto` y `default` dejan el esfuerzo sin especificar. Otros valores son `none`, `minimal`, `low`, `medium`, `high`, `xhigh` y `max`. No todos los modelos soportan todos los valores, y el catálogo puede no anunciar sus capacidades. Cambiar la configuración no envía una solicitud de generación facturable. El proveedor valida el soporte en las solicitudes posteriores. La API Responses usa `reasoning.effort`; la API Chat Completions usa `reasoning_effort`. Claude utiliza `output_config.effort` con pensamiento adaptativo. `none` solicita que se desactive el pensamiento en los modelos que lo soportan. No asuma que todos los modelos soportan un esfuerzo mayor o la desactivación del pensamiento. Consulte [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) y [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort).

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

Cambie a una cuenta existente utilizando `i plugins use chatgpt ACCOUNT_ID`. Utilice `i login chatgpt --account ACCOUNT_ID` para reautorizar una inscripción existente, o `i plugins logout chatgpt --account ACCOUNT_ID` para cerrar sesión en una cuenta específica. La reautorización valida la inscripción sin copiar los antiguos tokens rotativos. No modifica la cuenta en la conexión actual hasta que confirme el modelo.

Para gestionar el complemento de Claude Console, usa `claude` en lugar de `chatgpt`. La instalación y las actualizaciones requieren confirmación. Después de actualizar, vuelva a habilitar el complemento y iniciar sesión para evitar reemplazar silenciosamente una conexión activa. Si solo actualiza el asistente y desea mantener la misma cuenta, vuelva a habilitar el complemento y ejecute `i plugins reconnect chatgpt`. Esto mantiene la cuenta y la configuración del modelo actuales y vincula la conexión activa con el nuevo ejecutable verificado. No inicia sesión nuevamente ni envía una solicitud de generación. Durante la instalación, `i` muestra la búsqueda de la versión, los bytes descargados, la verificación del control de sumas y el progreso de guardado. Use `/cancel` para detener. La conexión expira después de 10 segundos, y cada descarga puede tardar hasta 120 segundos. Los fallos no desencadenan un reintento automático ni habilitan el complemento. Las descargas de GitHub utilizan las variables de entorno del proxy del terminal, no la configuración de proxy de su navegador. Si es necesario, comience con `HTTPS_PROXY=http://127.0.0.1:3067 i`, reemplazando la dirección y el puerto con sus propias configuraciones de proxy.

También puede modificar las reglas de proxy desde dentro de `i`. Por defecto, las reglas se guardan por usuario en `~/.i/config/proxy.json` (bajo `I_CONFIG_DIR` si está configurado), y no en el entorno de trabajo.

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

Las reglas se aplican en este orden: primero la regla para el uso concreto, luego la regla predeterminada del usuario y, por último, la configuración original o las variables de entorno. La opción explícita `no` nunca recurre al proxy del entorno. Los cambios solo afectan a operaciones posteriores: no interrumpen ni repiten las solicitudes activas. Configura por separado el navegador y cualquier servicio independiente que ya esté en ejecución. Se admiten HTTP, HTTPS, SOCKS5 y SOCKS5h. Las devoluciones de llamada de autorización locales no usan el proxy. Las URL del proxy no admiten usuarios, contraseñas, parámetros de consulta ni rutas para evitar que las credenciales lleguen al historial de comandos. Este comando forma parte de la aplicación: no expone la configuración del proxy ni las credenciales a los complementos WASM del espacio de trabajo. Quitar el complemento de la selección de instalación conserva los paquetes y las credenciales antiguos y no revoca la autorización remota. Para cerrar sesión, ejecuta `i plugins logout NAME` antes de desactivar o quitar el complemento. Si no se confirma la revocación remota, revisa la configuración de la cuenta en el proveedor.

El instalador descarga el archivo de conexión correspondiente desde el repositorio oficial de GitHub Releases. Verifica el SHA-256 proporcionado por GitHub, la lista de archivos ZIP permitidos y la plataforma de destino, y luego verifica el "digest" del ejecutable antes de cada invocación. Si no se encuentra ninguna plataforma compatible, el instalador se detiene en lugar de ejecutar scripts de instalación de terceros. El "checksum" verifica la integridad, no la identidad del proveedor. Los "paquetes de conexión" para macOS no están firmados con Developer ID ni están "notarizados". La instalación del complemento no modifica la variable PATH ni requiere privilegios de administrador.

Los desarrolladores pueden probar un paquete creado localmente con `i plugins install chatgpt --package /absolute/path/package.zip`. Después de la confirmación, el instalador sigue comprobando los miembros del archivo y la plataforma de destino. El complemento se inicia sin estar habilitado, y se comprueba su "digest" ejecutable antes de su uso. Un paquete local se etiqueta como `local-build`, pero no se verifica como una versión oficial. Instale solo el código nativo en el que confíe. Una confirmación de devolución de llamada del navegador no significa que el inicio de sesión esté completo. Espere a que el terminal finalice el intercambio de tokens, la verificación de identidad y la selección del modelo.

Los archivos `desktop.json` de nivel de usuario más antiguos aún pueden listar `search` y `find_files`, que ahora están integrados en el núcleo. Al cargar, el host elimina las entradas de búsqueda antiguas integradas que son reconocibles y vincula los antiguos trabajadores integrados del mismo directorio de configuración del usuario al ejecutable actual, sin modificar el archivo en el disco. Los nombres conflictivos para los trabajadores y complementos personalizados aún producen un error; el host no omite las comprobaciones de duplicados. Estos errores ocurren antes de las solicitudes del modelo, por lo que iniciar sesión nuevamente es innecesario.

### Diagnosticar fallos de generación

Las conexiones nativas de ChatGPT utilizan la API de Respuestas con `store:false` y `stream:true`, no las "Chat Completions". La terminal prefiere SSE, pero también acepta un objeto de Respuesta completo `application/json` si tiene `object:response`, un ID válido, `status:completed` y una salida válida. Sin un encabezado `Content-Type`, detecta SSE o JSON del cuerpo y luego aplica la misma validación. Rechaza explícitamente HTML u otros tipos de medios no soportados. Los diagnósticos de fallo muestran solo la categoría de formato y el número de bytes, no el cuerpo de la respuesta. No se aceptan objetos de "Chat Completions", JSON inválido y llamadas a herramientas incompletas. Un cambio en el formato de la respuesta no provoca que se reenvíe la solicitud. El servicio web local transmite SSE válido en vivo. Si el tipo de contenido está ausente o la respuesta es JSON, valida la respuesta completa antes de proporcionar SSE válido al navegador. Este camino de compatibilidad espera a que se complete y no reenvía automáticamente. Un error HTTP 400 local en el navegador no necesariamente indica el estado upstream: los diagnósticos distinguen los errores HTTP upstream, fallos de red y fallos de formato.

En caso de fallo, el diagnóstico identifica la etapa: preparación de la solicitud, autorización, HTTP, formato de respuesta o análisis del flujo. Los diagnósticos de HTTP solo incluyen el estado, los códigos de error conocidos o los nombres de los parámetros, y un ID de solicitud limitado. No muestran errores, mensajes o tokens brutos. Los errores de red o de formato no eliminan tu inicio de sesión ni cambian automáticamente las cuentas, los modelos o las opciones de facturación de la API.

## Configuración y credenciales

- `<config_dir>/native-plugins.json` almacena la ruta de instalación, el hash y el estado habilitado, pero no los tokens de OAuth.
- `<data_dir>/connections/accounts/NAME` almacena las credenciales del plugin; los plugins WASM para el espacio de trabajo no las reciben.
- El menú de inicio de sesión completa la configuración del modelo. No es necesario que escriba `chatgpt.json`.
- macOS y Linux utilizan directorios y permisos de archivos privados. Windows hereda el Control de Acceso Basado en Usuarios (ACL) del directorio del usuario.
- La aplicación no cifra las credenciales ni las almacena en un llavero del sistema. Los programas nativos maliciosos que se ejecutan con el mismo usuario aún podrían leerlas.

No coloque estas carpetas en espacios de trabajo, carpetas compartidas, sistemas de control de versiones o copias de seguridad públicas. Las conexiones directas de la API siguen funcionando sin necesidad de un plugin de inicio de sesión.

## Vincular un navegador

Después de instalar y activar el complemento, ejecute el siguiente comando en la misma computadora que utiliza el navegador:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Para Claude, usa `claude` en lugar de `chatgpt` en el comando anterior. Mantenga el terminal abierto y abra el enlace de emparejamiento impreso. Confirme el complemento, la cuenta y el modelo en la página, y luego guarde. El emparejamiento crea primero un borrador sin guardar y no reemplaza automáticamente una conexión existente. Utilice un nuevo enlace después de reiniciar el servicio. No comparta los enlaces de emparejamiento: cada uno contiene un secreto de capacidad local.

El servicio solo escucha en `127.0.0.1`, está vinculado a la página de origen especificada y no utiliza el enrutamiento para URLs arbitrarias. Las páginas HTTPS que acceden a la red local pueden requerir permisos de red local. La asociación no elude los permisos del navegador ni las políticas CORS. Un teléfono no puede ejecutar el plugin nativo por sí solo, y eliminar la conexión del navegador no cierra la sesión de la cuenta nativa.

## Limitaciones de ChatGPT y solución de problemas

El plugin utiliza el registro dinámico de clientes OAuth oficial, PKCE, una identidad de host separada y un almacenamiento de credenciales separado. No lee su archivo `~/.codex/auth.json` personal ni utiliza identificadores de cliente propietarios. Consulte la [documentación oficial de inicio de sesión](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) y las secciones sobre [perfiles y sesiones](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

Inference utiliza la API pública de Respuestas, no la API privada `backend-api`. OpenAI determina la elegibilidad, las políticas del espacio de trabajo y los límites de autorización; iniciar sesión no garantiza el acceso a Inference. La autorización basada en suscripción utiliza configuraciones fijas de `store: false` y `stream: true`. La versión de previsualización actual ignora la configuración del número máximo de tokens de salida; consulte [limitaciones de la previsualización](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Los fallos se distinguen en el intercambio de códigos de autorización, la renovación de la sesión y los errores de red. Los diagnósticos muestran los códigos de estado HTTP sin mostrar las respuestas del punto final del token, las credenciales o los códigos de autorización. Las solicitudes de autorización o de modelo fallidas no se reintentan automáticamente. Si el intercambio de códigos es rechazado, inicie nuevamente el proceso de inicio de sesión y verifique la versión, la hora del sistema, la red y la configuración del proxy. El código de estado por sí solo no le indica por qué fue rechazado.

## Limitaciones de Claude Console

Siga la [guía de instalación oficial](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) para instalar `ant` y asegurarse de que esté en la variable PATH. El paquete de conexión no incluye `ant` ni ejecuta su script de instalación por usted. Luego, utilice `i login claude`.

El complemento utiliza un entorno HOME aislado, `ANTHROPIC_CONFIG_DIR`, y su propio perfil. No lee códigos de Claude existentes ni perfiles personales. Actualmente, admite un solo perfil de consola. La reautorización puede cambiar el espacio de trabajo, y las solicitudes posteriores pueden utilizar el nuevo espacio de trabajo; inicie una nueva sesión si es necesario. **Claude se factura a través de la API de Console, no con la cuota de tu suscripción Pro o Max.**

## Alcance de la verificación

Las pruebas automatizadas utilizan directorios aislados y configuraciones locales de OAuth/TLS. No utilizan tokens personales reales, no autorizan el acceso en tu nombre ni envían solicitudes de facturación. El paso de pruebas sin conexión no prueba que un proveedor otorgará acceso a una cuenta real. Los componentes auxiliares mantienen sus protocolos internos de trabajador y servicio; las operaciones visibles para el usuario pasan a través de `i`.
