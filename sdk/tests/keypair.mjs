import { expect } from "chai";
import { Alembic_program_init, Keypair } from "crate";
Alembic_program_init();

describe("Keypair", function () {
  it("works", () => {
    const keypair = new Keypair();
    let bytes = keypair.toBytes();
    expect(bytes).to.have.length(64);

    const recoveredKeypair = Keypair.fromBytes(bytes);
    expect(keypair.pubkey().equals(recoveredKeypair.pubkey()));
  });
});
