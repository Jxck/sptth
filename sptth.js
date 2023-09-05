#!/usr/bin/env node

import { readFile } from "fs/promises";
import { resolve } from "path";
import redbird from "redbird";
import { parse } from "yaml";

function help() {
  console.log(
    `
sptth is the HTTPS reverse proxy cli (https <-> sptth)
$ sptth proxy.yaml

\`\`\`yaml
sptth:
  hosts:
    alice.example:
      key: ./cert/alice.example-key.pem
      cert: ./cert/alice.example.pem
      url: http://127.0.0.1:3000
    bob.example:
      key: ./cert/bob.example-key.pem
      cert: ./cert/bob.example.pem
      url: http://127.0.0.1:4000
\`\`\`
  `.trim(),
  );
  process.exit(0);
}

// CLI option
const arg = process.argv.at(2);

if (arg === undefined || arg.startsWith("-")) {
  help();
}

const yaml = resolve(arg);
const file = await readFile(yaml, { encoding: "utf-8" });
const { hosts, config } = parse(file).sptth;

const proxy = redbird(config);

console.log(`starting reverse https proxy on port ${config.port}`);

Object.entries(hosts).forEach(([key, setting]) => {
  console.log(`> https://${key} => ${setting.url}`);
  proxy.register(key, setting.url, {
    ssl: {
      key: setting.key,
      cert: setting.cert,
    },
  });
});
