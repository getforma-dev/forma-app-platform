import type { WireRecordComponent } from './types';
export type { WireRecordComponent, WireRecordFilter } from './types';

/** Imperative counterpart of the native renderer; never interprets record text as markup. */
export function renderRecordComponents(components: WireRecordComponent[], doc: Document = document): HTMLElement {
  const node = (tag: string, text = '', className = '') => {const n = doc.createElement(tag); n.textContent = text; n.className = className; return n;};
  const outer = node('div', '', 'record-components');
  for (const p of components) {
    const section = node('section','','record-component view'); section.dataset.recordComponent=p.id;section.dataset.state=p.state;section.dataset.filterEpoch=String(p.filter_epoch);
    section.append(node('h2',p.title));
    if (p.has_filters) {
      const form=doc.createElement('form');form.className='record-filter-form';form.method='post';form.dataset.recordFilters=p.id;
      for (const f of p.filters) {
        const fieldset=doc.createElement('fieldset');fieldset.className='record-filter';fieldset.dataset.recordFilter=f.id;fieldset.append(node('legend',f.label));
        const input=(type:string,value:string,marker:'filterValue'|'filterStart'|'filterEnd',label:string)=>{const l=doc.createElement('label');l.append(doc.createTextNode(label));const i=doc.createElement('input');i.type=type;i.value=value;i.dataset[marker]=f.id;if(type==='search'){i.maxLength=200;i.autocomplete='off';}l.append(i);fieldset.append(l);};
        if(f.is_text)input('search',f.value,'filterValue','Contains ');
        if(f.is_date){input('date',f.start,'filterStart','From ');input('date',f.end,'filterEnd','Before ');}
        if(f.is_enum){const l=doc.createElement('label');l.append(doc.createTextNode('Equals '));const select=doc.createElement('select');select.dataset.filterValue=f.id;const all=doc.createElement('option');all.value='';all.textContent='All';select.append(all);for(const o of f.options){const option=doc.createElement('option');option.value=o.value;option.textContent=o.label;option.selected=o.selected;select.append(option);}l.append(select);fieldset.append(l);}
        form.append(fieldset);
      }
      const apply=doc.createElement('button');apply.type='submit';apply.disabled=true;apply.dataset.recordFilterEnable='';apply.textContent='Apply filters';const reset=doc.createElement('button');reset.type='button';reset.disabled=true;reset.dataset.recordFilterEnable='';reset.dataset.recordFilterReset='';reset.textContent='Reset filters';form.append(apply,reset);section.append(form);
    }
    const status=node('p',p.message,'state');status.setAttribute('role','status');status.setAttribute('aria-live','polite');section.append(status);
    if(p.has_summary){const value=node('p',p.summary,'record-metric');value.title=p.summary_raw;section.append(value);}
    if(p.has_budget){const budget=node('div','','record-budget');budget.append(node('p',p.budget),node('p',p.ratio),node('p',p.remaining));section.append(budget);}
    if(p.has_groups){const figure=node('figure','','record-groups'),list=node('ol');for(const g of p.groups){const row=node('li'),label=node('span',g.label),value=node('strong',g.value);value.title=g.raw;const track=doc.createElementNS('http://www.w3.org/2000/svg','svg');track.setAttribute('class','record-bar-track');track.setAttribute('aria-hidden','true');const bar=doc.createElementNS('http://www.w3.org/2000/svg','rect');bar.setAttribute('class','record-bar');bar.setAttribute('height','8');if(/^(?:100|[0-9]{1,2})%$/.test(g.width))bar.setAttribute('width',g.width);track.append(bar);row.append(label,value,track);list.append(row);}figure.append(list,node('figcaption','Values cover all matching saved records. Bars show relative magnitude.'));section.append(figure);}
    if(p.has_table){const box=node('div','','shared-table'),table=node('table'),head=node('thead'),hr=node('tr'),body=node('tbody');box.setAttribute('role','region');box.setAttribute('aria-label',p.title);box.tabIndex=0;for(const c of p.columns){const th=node('th',c.label);th.setAttribute('scope','col');hr.append(th);}head.append(hr);for(const r of p.rows){const tr=node('tr');tr.dataset.recordVersion=r.version;for(const c of r.cells){const td=node('td',c.text);td.title=c.raw;tr.append(td);}body.append(tr);}table.append(head,body);box.append(table);section.append(box);}
    if(p.has_receipt){const details=node('details','','receipt'),summary=node('summary','Calculation details');details.append(summary,node('code',p.receipt_digest),node('pre',p.receipt_json));section.append(details);}
    outer.append(section);
  }
  return outer;
}
