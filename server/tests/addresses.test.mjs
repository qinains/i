import test from 'node:test';
import assert from 'node:assert/strict';
import {isPublicAddress,resolvePublic} from '../addresses.mjs';
test('deny private, special, mapped and transition addresses; accept public addresses', async () => {
    for (const ip of ['127.0.0.1','10.1.2.3','100.64.0.1','169.254.0.1','172.16.0.1','192.168.1.1','192.0.0.1','198.18.0.1','198.51.100.1','203.0.113.1','224.0.0.1','0.0.0.0','::1','fe80::1','fc00::1','::ffff:127.0.0.1','::ffff:8.8.8.8','2001:db8::1','2002:7f00:1::','2001::1','invalid']) assert.equal(isPublicAddress(ip),false,ip);
    for (const ip of ['8.8.8.8','1.1.1.1','2606:4700:4700::1111']) assert.equal(isPublicAddress(ip),true,ip);
    await assert.rejects(resolvePublic('api.example',async () => [{address:'8.8.8.8',family:4},{address:'127.0.0.1',family:4}]),/public/i);
});
