<script lang="ts">
  import { parse, type Part } from "./markdown";

  let { text }: { text: string } = $props();
  const blocks = $derived(parse(text));
</script>

{#snippet inline(parts: Part[])}
  {#each parts as p, i (i)}
    {#if p.code}<code>{p.text}</code>{:else if p.bold}<strong>{p.text}</strong>{:else if p.em}<em>{p.text}</em
      >{:else if p.strike}<s>{p.text}</s>{:else}{p.text}{/if}
  {/each}
{/snippet}

<div class="md">
  {#each blocks as b, i (i)}
    {#if b.kind === "h"}
      <svelte:element this={`h${Math.min(b.level + 1, 6)}`}>{@render inline(b.parts)}</svelte:element>
    {:else if b.kind === "p"}
      <p>{@render inline(b.parts)}</p>
    {:else if b.kind === "list"}
      <ul>
        {#each b.items as item, j (j)}
          <li style:margin-left="{item.depth * 18}px">
            <span class="marker" aria-hidden="true">{item.marker}</span>
            <span>{@render inline(item.parts)}</span>
          </li>
        {/each}
      </ul>
    {:else if b.kind === "pre"}
      <pre><code>{b.text}</code></pre>
    {:else if b.kind === "quote"}
      <blockquote>{@render inline(b.parts)}</blockquote>
    {:else if b.kind === "hr"}
      <hr />
    {:else if b.kind === "table"}
      <div class="table">
        <table>
          <thead>
            <tr>
              {#each b.head as cell, c (c)}
                <th style:text-align={b.align[c]}>{@render inline(cell)}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each b.rows as row, r (r)}
              <tr>
                {#each row as cell, c (c)}
                  <td style:text-align={b.align[c]}>{@render inline(cell)}</td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/each}
</div>

<style>
  .md :global(h2),
  .md :global(h3),
  .md :global(h4),
  .md :global(h5),
  .md :global(h6) {
    margin: 14px 0 4px;
    font-size: 13px;
  }
  .md :global(h2) {
    font-size: 15px;
  }
  .md :global(h3) {
    font-size: 14px;
  }
  p {
    margin: 8px 0;
    white-space: pre-line;
  }
  ul {
    margin: 6px 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: 6px;
    margin: 3px 0;
  }
  .marker {
    flex: none;
    min-width: 1.2em;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  code {
    padding: 0 4px;
    border-radius: 4px;
    background: var(--hover);
    font: 12px ui-monospace, Menlo, Consolas, monospace;
  }
  pre {
    margin: 8px 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--hover);
    overflow-x: auto;
  }
  pre code {
    padding: 0;
    background: none;
  }
  blockquote {
    margin: 8px 0;
    padding: 2px 0 2px 10px;
    border-left: 3px solid var(--border);
    color: var(--muted);
  }
  hr {
    margin: 12px 0;
    border: 0;
    border-top: 1px solid var(--border);
  }
  /* Wide tables scroll sideways inside the answer instead of widening it. */
  .table {
    margin: 8px 0;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    padding: 5px 10px;
    text-align: left;
    vertical-align: top;
    border-bottom: 1px solid var(--border);
  }
  th {
    background: var(--hover);
    font-weight: 600;
    white-space: nowrap;
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
  tbody tr:nth-child(even) td {
    background: rgba(127, 127, 127, 0.07);
  }
</style>
