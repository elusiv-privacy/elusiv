import {
    Keypair,
    PublicKey
} from '@solana/web3.js';
import * as fs from 'mz/fs';
import * as yaml from 'yaml';
import * as path from 'path';
import * as os from 'os';

export async function createKeypairFromFile(filePath: string): Promise<Keypair> {
    const secretKeyString = await fs.readFile(filePath, { encoding: 'utf8' });
    const secretKey = Uint8Array.from(JSON.parse(secretKeyString));
    return Keypair.fromSecretKey(secretKey);
}

async function getConfig(): Promise<any> {
    const CONFIG_FILE_PATH = path.resolve(os.homedir(), '.config', 'solana', 'cli', 'config.yml');
    const configYml = await fs.readFile(CONFIG_FILE_PATH, { encoding: 'utf8' });
    return yaml.parse(configYml);
}
  
export async function getPayer(): Promise<Keypair> {
    try {
        const config = await getConfig();
        if (!config.keypair_path) throw new Error('Missing keypair path');
        return await createKeypairFromFile(config.keypair_path);
    } catch (err) {
        console.warn('Failed to create keypair from CLI config file, falling back to new random keypair');
        return Keypair.generate();
    }
}


export function exportPubKeys(program_id: PublicKey, bank_account: PublicKey) {
    const data = {
      "program_id": program_id.toBase58(),
      "main_account": bank_account.toBase58(),
    };
  
    fs.writeFileSync("dist/program/pubkeys.json", JSON.stringify(data), { encoding:'utf8', flag:'w' });
}