import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DexscreenerEscrow } from "../target/types/dexscreener_escrow";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import bs58 from "bs58"
describe("dex_raise", () => {
  // Load admin key from base58
  const admin = Keypair.fromSecretKey(bs58.decode(""));

  const feeWallet = new PublicKey("EwP2v1nmR5j5YoqeXPHkhNBJaGBRJs2HLD6FVwr9q7wN")

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
      .initialize(feeWallet)
      .accountsStrict({
        config: configPDA,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin]) 
      .rpc();

    console.log("✅ Init tx:", tx);
  });
});
