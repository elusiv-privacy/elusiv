import { setup_program_account } from './setup';
import {
    createKeypairFromFile,
    exportPubKeys
} from './utils';

async function main() {
    const SIZE = 1000;
    const program = await createKeypairFromFile("dist/program/elusiv-keypair.json");
    const bank_account = await setup_program_account(program.publicKey, SIZE);
    exportPubKeys(program.publicKey, bank_account);
}

main().then(
    () => process.exit(),
    err => {
        console.error(err);
        process.exit(-1);
    },
);