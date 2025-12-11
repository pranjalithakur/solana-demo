import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";

describe("vuln_dapp", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.VulnDapp as Program;

  it("initializes a pool (intentionally unsafe)", async () => {
    // Example test scaffolding – left minimal on purpose to keep focus on the on-chain code.
    // A real project would derive PDAs and exercise all instructions here.
  });
});
