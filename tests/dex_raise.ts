import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DexscreenerEscrow } from "../target/types/dexscreener_escrow";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import bs58 from "bs58"
describe("dex_raise", () => {
  // Load admin key from base58
  const admin = Keypair.fromSecretKey(bs58.decode("4r7H961ANWxJHhGoCgvBgPNPNZdmnRczVfzdiyr7p7pPSGxmocJmzMSSbuarRVkN8rQfn7WRBPZrfsqKuRYpMEho"));

  // Set admin as provider
  anchor.setProvider(
    new anchor.AnchorProvider(anchor.AnchorProvider.env().connection, new anchor.Wallet(admin), {
      preflightCommitment: "confirmed",
    })
  );

  const program = anchor.workspace.DexscreenerEscrow as Program<DexscreenerEscrow>;

  const configPDA = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId
  )[0];

  it("Is initialized!", async () => {
    const tx = await program.methods
      .initialize(100, admin.publicKey)
      .accountsStrict({
        config: configPDA,
        admin: admin.publicKey,
        treasury: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin]) 
      .rpc();

    console.log("✅ Init tx:", tx);
  });
});
