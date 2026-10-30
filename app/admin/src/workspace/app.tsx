import { h, mount, createShow, createList } from '@getforma/core';

// The protected workspace entry: rendered only for a live workspace session.
// APP-076 G9: it lists the saved works the member may open now, each a plain
// link to its work page and, when the member may write its records, to the
// record page of a fresh record. The Rust handler resolves every item under
// the session's fenced, scoped transaction; this page only lays them out. An
// empty listing says only that there is nothing to open yet.
type RecordLink = { id: string; label: string; href: string };
type Item = { id: string; title: string; href: string; hasRecords: boolean; records: RecordLink[] };
type WorkspaceProps = {
  csrfSignout: string;
  csrfSignoutAll: string;
  hasItems: boolean;
  items: Item[];
};

function Workspace(props: WorkspaceProps) {
  return (
    <main class="page">
      <h1>Workspace</h1>
      <p class="lede">You are signed in.</p>
      <h2>Saved work</h2>
      {props.hasItems ? null : <p>Nothing to open yet.</p>}
      {createShow(() => props.hasItems, () => (
        <ul class="openable">
          {createList(() => props.items, item => item.id, item => (
            <li>
              <a href={item.href}>{item.title}</a>
              <p class="label">Seeded definition, retrieved from the library. Sources seeded by the operator.</p>
              {createShow(() => item.hasRecords, () => (
                <ul>
                  {createList(() => item.records, record => record.id, record => (
                    <li>
                      <a href={record.href}>{record.label}</a>
                    </li>
                  ))}
                </ul>
              ))}
            </li>
          ))}
        </ul>
      ))}
      <form method="post" action="/auth/signout">
        <input type="hidden" name="csrf" value={props.csrfSignout} />
        <button type="submit">Sign out</button>
      </form>
      <form method="post" action="/auth/signout-all">
        <input type="hidden" name="csrf" value={props.csrfSignoutAll} />
        <button type="submit">Sign out everywhere</button>
      </form>
    </main>
  );
}

mount(() => Workspace({ csrfSignout: '', csrfSignoutAll: '', hasItems: false, items: [] }), '#app');
