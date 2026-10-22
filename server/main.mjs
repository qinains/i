import {createRelayServer} from './relay.mjs';
try {
    const server = await createRelayServer({token:process.env.HARNESS_RELAY_TOKEN,origin:process.env.HARNESS_PUBLIC_ORIGIN,
        endpoints:JSON.parse(process.env.HARNESS_UPSTREAMS ?? '[]'),staticRoot:process.env.HARNESS_STATIC_ROOT,
        basePath:process.env.HARNESS_BASE_PATH ?? '/'});
    server.listen(8080,'127.0.0.1',() => process.stdout.write('Harness relay listening on 127.0.0.1:8080\n'));
} catch {process.stderr.write('Relay startup failed. Check required configuration (see server/README.md).\n'); process.exitCode = 1;}
