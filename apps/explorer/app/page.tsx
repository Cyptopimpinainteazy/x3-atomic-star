// X3 Chain Explorer — the head of the chain, read at request time.
//
// This page used to be a heading and a sentence: it satisfied the launch gate's explorer
// criterion while showing no chain data at all, because that criterion asked the body to name
// itself as the explorer and nothing more (measured 2026-09-26, TICKET-140). It now reads the
// finalized head from an X3 JSON-RPC endpoint and renders the number and hash it was given.
//
// It fails closed. If the endpoint does not answer, or answers with something that is not a
// finalized head, the page says so and renders *no* height: a number that was not read from a
// chain is worse than no number, and the launch gate checks for the marker below when it cannot
// read the chain itself.
//
//   X3_EXPLORER_RPC   — endpoint to read (falls back to X3_RPC_URL, then loopback 9944)
//
// The `data-x3-explorer` / `data-x3-explorer-error` attributes are the criterion's hooks:
// `chain-head` with the height in the body means "this page read the chain", and
// `rpc-unreachable` means "this page refused to invent one".

export const dynamic = 'force-dynamic'

const RPC_URL =
  process.env.X3_EXPLORER_RPC ??
  process.env.X3_RPC_URL ??
  'http://127.0.0.1:9944'

type Head =
  | { ok: true; number: number; hash: string; fetchedAt: string }
  | { ok: false; error: string; fetchedAt: string }

async function rpc(method: string, params: unknown[]): Promise<unknown> {
  const response = await fetch(RPC_URL, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    cache: 'no-store',
    signal: AbortSignal.timeout(5000),
  })
  if (!response.ok) {
    throw new Error(`${method}: HTTP ${response.status}`)
  }
  const body = (await response.json()) as { result?: unknown; error?: { message?: string } }
  if (body.error) {
    throw new Error(`${method}: ${body.error.message ?? 'rpc error'}`)
  }
  if (body.result === undefined || body.result === null) {
    throw new Error(`${method}: empty result`)
  }
  return body.result
}

async function readFinalizedHead(): Promise<Head> {
  const fetchedAt = new Date().toISOString()
  try {
    const hash = await rpc('chain_getFinalizedHead', [])
    if (typeof hash !== 'string' || hash.length === 0) {
      throw new Error('chain_getFinalizedHead: expected a block hash')
    }
    // Substrate's `chain_getHeader` answers with the header as JSON (`number` in hex), not a
    // SCALE blob — `chain_getBlock` is the call that returns hex-encoded bytes. Anything else
    // is treated as unreadable rather than guessed at.
    const header = (await rpc('chain_getHeader', [hash])) as { number?: unknown }
    if (typeof header?.number !== 'string') {
      throw new Error('chain_getHeader: the header carries no number')
    }
    const number = Number.parseInt(header.number, 16)
    if (!Number.isSafeInteger(number)) {
      throw new Error(`chain_getHeader: unreadable block number ${header.number}`)
    }
    return { ok: true, number, hash, fetchedAt }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
      fetchedAt,
    }
  }
}

export default async function ExplorerPage() {
  const head = await readFinalizedHead()
  // The launch gate pins this page by URL, so the URL is part of the page's identity.
  const explorerName = process.env.X3_EXPLORER_NAME ?? 'X3 Chain Explorer'

  return (
    <main>
      <h1>{explorerName}</h1>
      <p>Block explorer for X3 Chain</p>
      {head.ok ? (
        <section data-x3-explorer="chain-head">
          <h2>Finalized head</h2>
          <p>
            Block <strong data-x3-explorer-height={head.number}>#{head.number}</strong>
          </p>
          <dl>
            <dt>Block hash</dt>
            <dd>{head.hash}</dd>
            <dt>Read from</dt>
            <dd>{RPC_URL}</dd>
            <dt>Read at</dt>
            <dd>{head.fetchedAt}</dd>
          </dl>
          <p>
            This page shows the finalized head it read from that endpoint. It does not index
            history: a block, transaction or account view is not implemented yet.
          </p>
        </section>
      ) : (
        <section data-x3-explorer-error="rpc-unreachable">
          <h2>Chain not reachable</h2>
          <p>
            No finalized head could be read from <code>{RPC_URL}</code>: {head.error}
          </p>
          <p>
            This page refuses to display a height it did not read from a chain. Point
            <code> X3_EXPLORER_RPC </code> at a running X3 node and reload.
          </p>
          <p>Checked at {head.fetchedAt}.</p>
        </section>
      )}
    </main>
  )
}
