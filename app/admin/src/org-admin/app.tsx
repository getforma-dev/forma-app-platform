import { h, mount, createShow, createList } from '@getforma/core';
import { SiteFooter } from '../site/footer';

// D4 (trial plan) and ADR-021 (organization roles): the organization's own
// admin area, `GET /admin`. The Rust handler (`org_admin`) renders it only
// for a member whose role is owner, admin or manager; anyone else gets the
// uniform 404 before any member data is read. Every value is a typed prop
// computed in Rust: the role select of a row offers only the roles the
// caller may set on that member (the server enforces the ladder regardless).
// Change role and Remove are plain forms: the synchronizer token, a required
// confirmation where it matters, no script. The server's answer comes back
// as `notice`.
type RoleOption = { value: string; label: string; selected: boolean };
type Member = {
  id: string;
  label: string;
  note: string;
  status: string;
  role: string;
  roleCode: string;
  changeable: boolean;
  /** The inverse of `changeable`: the role is shown as text. */
  fixedRole: boolean;
  options: RoleOption[];
  confirmRole: boolean;
  /** "Role for <name>": the select's accessible name. */
  roleSelectLabel: string;
  roleAction: string;
  csrfRole: string;
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
  /** The list was read and has no member. */
  empty: boolean;
  members: Member[];
  csrfSignout: string;
};

function Admin(props: AdminProps) {
  return (
    <div class="site">
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
        <ul class="org-admin-legend" aria-label="Roles">
          <li><strong>Owner</strong>: everything, including other owners and admins.</li>
          <li><strong>Admin</strong>: manages members, managers and viewers.</li>
          <li><strong>Manager</strong>: manages members and viewers.</li>
          <li><strong>Member</strong>: builds and shares apps.</li>
          <li><strong>Viewer</strong>: read-only.</li>
        </ul>
        <p class="org-admin-helper">Helpers such as assistants: make them Members and give them Reviewer on the apps they help with.</p>
        {createShow(() => props.empty, () => <p>No members yet.</p>)}
        {createShow(() => props.hasMembers, () => (
          <div class="org-admin-table-wrap">
            <table class="org-admin-table">
              <thead>
                <tr>
                  <th scope="col">Name</th>
                  <th scope="col">Role</th>
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
                  <tr data-member={member.id} data-role={member.roleCode}>
                    <th scope="row">{member.label}<span class="org-admin-role">{member.role}</span></th>
                    <td>
                      {createShow(() => member.changeable, () => (
                        <form method="post" action={member.roleAction} class="org-admin-role-form">
                          <input type="hidden" name="csrf" value={member.csrfRole} />
                          <input type="hidden" name="from" value={member.roleCode} />
                          <select name="role" aria-label={member.roleSelectLabel}>
                            {createList(() => member.options, option => option.value, option => (
                              <option value={option.value} selected={option.selected}>{option.label}</option>
                            ))}
                          </select>
                          {createShow(() => member.confirmRole, () => (
                            <label><input type="checkbox" name="confirm" value="role" />I understand this changes owner or admin access</label>
                          ))}
                          <button type="submit">Change role</button>
                        </form>
                      ))}
                      {createShow(() => member.fixedRole, () => <span>{member.role}</span>)}
                    </td>
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
      <SiteFooter />
    </div>
  );
}

mount(() => Admin({ organization: '', summary: '', hasNotice: false, notice: '', noticeKind: '', hasMembers: false, empty: false, members: [], csrfSignout: '' }), '#app');
