// Smoke-only HTTP fixture. Normal runtime builds cannot override the Jupiter endpoint.
import { createServer } from "node:http";
const usdc = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const sol = "So11111111111111111111111111111111111111112";
let contexts = [];
createServer(async (req, res) => {
  const url = new URL(req.url, "http://127.0.0.1:18081");
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  const body = chunks.length
    ? JSON.parse(Buffer.concat(chunks).toString())
    : {};
  let result;
  if (url.pathname === "/health") result = { ok: true };
  else if (url.pathname === "/contexts") result = contexts;
  else if (url.pathname === "/price/v3")
    result = {
      [sol]: { usdPrice: 150, blockId: 1000000 },
      [usdc]: { usdPrice: 1, blockId: 1000000 },
    };
  else if (url.pathname === "/rpc") {
    if (
      !["getGenesisHash", "getSlot", "getMultipleAccounts"].includes(
        body.method,
      )
    ) {
      res.writeHead(400).end();
      return;
    }
    result = {
      jsonrpc: "2.0",
      id: 1,
      result:
        body.method === "getGenesisHash"
          ? "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"
          : body.method === "getMultipleAccounts"
            ? {
                context: { slot: 1000000 },
                value: [9, 6].map((decimals) => ({
                  owner: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                  data: {
                    parsed: {
                      type: "mint",
                      info: {
                        decimals,
                        supply: "1000000",
                        isInitialized: true,
                        mintAuthority: null,
                        freezeAuthority: null,
                      },
                    },
                  },
                })),
              }
            : 1000000,
    };
  } else if (url.pathname === "/swap/v2/order") {
    if (url.searchParams.has("taker")) {
      res.writeHead(400).end();
      return;
    }
    const q = Object.fromEntries(url.searchParams);
    const amount = BigInt(q.amount);
    const out =
      q.inputMint === usdc ? (amount * 1000n) / 150n : (amount * 150n) / 1000n;
    result = {
      inputMint: q.inputMint,
      outputMint: q.outputMint,
      inAmount: amount.toString(),
      outAmount: out.toString(),
      otherAmountThreshold: (
        (out * (10000n - BigInt(q.slippageBps))) /
        10000n
      ).toString(),
      swapMode: "ExactIn",
      transaction: null,
    };
  } else if (url.pathname === "/chat") {
    const context = JSON.parse(body.messages[1].content);
    contexts.push(context);
    const position = context.open_positions[0];
    const decision = {
      action: position ? "close" : "open",
      notional_usdc: position ? "0" : "100",
      position_id: position?.id ?? null,
      rationale: "Smoke provider decision from market and trade memory",
      lessons: `Read ${context.trade_history.length} verified outcomes`,
    };
    const report = {
      summary: `${context.agent_role} analyzed live evidence`,
      evidence: [`Read ${context.trade_history.length} verified outcomes`],
      risks: ["Test evidence only"],
      assessment: "proceed",
    };
    result = {
      choices: [
        {
          message: {
            content: JSON.stringify(
              context.agent_role === "strategy_evaluation" ? decision : report,
            ),
          },
        },
      ],
    };
  } else {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { "content-type": "application/json" });
  res.end(JSON.stringify(result));
}).listen(18081, "127.0.0.1");
