import { h, mount, createShow, createList } from '@getforma/core';

// D4 (trial plan): the organization's own admin area, `GET /admin`. The Rust
// handler (`org_admin`) renders it only for a member who holds
// `admin_policy` in the organization; anyone else gets the uniform 404 before
// any member data is read. Every value is a typed prop computed in Rust.
// Remove is a plain form: the synchronizer token, a required confirmation,
// no script. The server's answer to it comes back as `notice`.
type Member = {
  id: string;
  label: string;
  note: string;
  status: string;
  role: string;
  consent: string;
  lastActive: string;
  apps: string;
  works: string;
  records: string;
  uploads: string;
  removable: boolean;
  action: string;
  csrf: string;
  confirmLabel: string;
};
type AdminProps = {
  organization: string;
  summary: string;
  hasNotice: boolean;
  notice: string;
  noticeKind: string;
  hasMembers: boolean;
  members: Member[];
  csrfSignout: string;
};

function Admin(props: AdminProps) {
  return (
    <main class="page org-admin">
      <a class="forma-brand" href="/canvas" aria-label="Forma canvas"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">admin</span></a>
      <nav class="org-admin-nav" aria-label="Admin">
        <a href="/canvas">Back to the canvas</a>
        <a href="mailto:feedback@getforma.dev">Feedback</a>
      </nav>
      <h1>{props.organization}</h1>
      <p class="lede">{props.summary}</p>
      {createShow(() => props.hasNotice, () => (
        <p class="org-admin-notice" role="status" data-kind={props.noticeKind}>{props.notice}</p>
      ))}
      <h2>Members</h2>
      {props.hasMembers ? null : <p>No members yet.</p>}
      {createShow(() => props.hasMembers, () => (
        <div class="org-admin-table-wrap">
          <table class="org-admin-table">
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col">Note</th>
                <th scope="col">Status</th>
                <th scope="col">Consent</th>
                <th scope="col">Last active</th>
                <th scope="col">Apps</th>
                <th scope="col">Works</th>
                <th scope="col">Records</th>
                <th scope="col">Uploads</th>
                <th scope="col">Remove</th>
              </tr>
            </thead>
            <tbody>
              {createList(() => props.members, member => member.id, member => (
                <tr data-member={member.id}>
                  <th scope="row">{member.label}<span class="org-admin-role">{member.role}</span></th>
                  <td>{member.note}</td>
                  <td>{member.status}</td>
                  <td>{member.consent}</td>
                  <td>{member.lastActive}</td>
                  <td class="org-admin-count">{member.apps}</td>
                  <td class="org-admin-count">{member.works}</td>
                  <td class="org-admin-count">{member.records}</td>
                  <td class="org-admin-count">{member.uploads}</td>
                  <td>
                    {createShow(() => member.removable, () => (
                      <form method="post" action={member.action} class="org-admin-remove">
                        <input type="hidden" name="csrf" value={member.csrf} />
                        <label><input type="checkbox" name="confirm" value="remove" required />{member.confirmLabel}</label>
                        <button type="submit">Remove</button>
                      </form>
                    ))}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
      <form method="post" action="/auth/signout">
        <input type="hidden" name="csrf" value={props.csrfSignout} />
        <button type="submit">Sign out</button>
      </form>
    </main>
  );
}

mount(() => Admin({ organization: '', summary: '', hasNotice: false, notice: '', noticeKind: '', hasMembers: false, members: [], csrfSignout: '' }), '#app');
