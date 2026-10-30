// APP-103: the clock widget, ported from our own forma-generative-ui
// `registry/artifacts/retro-digital-clock/1.2.0` (the LED case, the glass,
// the blinking colon; its time zone and 12/24 h selects). Time is read
// locally with `Intl.DateTimeFormat`; nothing is fetched.
import { el } from '../render';
import { type WidgetCard } from './card';
import { ZONES, zoneName, type ClockState } from './state';

type Parts = { hours: string; minutes: string; seconds: string; period: string; date: string };

export function clockParts(now: Date, zone: string, hour12: boolean): Parts {
  const timeZone = zone === 'local' ? undefined : zone;
  const parts = new Intl.DateTimeFormat('en-US', {
    timeZone, hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: hour12 ? 'h12' : 'h23',
  }).formatToParts(now);
  const part = (type: string) => parts.find((p) => p.type === type)?.value ?? '';
  const date = new Intl.DateTimeFormat('en-US', { timeZone, weekday: 'short', month: 'short', day: 'numeric' }).format(now);
  return {
    hours: part('hour').padStart(2, '0'),
    minutes: part('minute'),
    seconds: part('second'),
    period: hour12 ? part('dayPeriod').toUpperCase() : '',
    date: date.toUpperCase(),
  };
}

export function mountClock(widget: WidgetCard): () => void {
  const document_ = widget.card.ownerDocument;
  const hours = el(document_, 'span', { class: 'clk-hours' });
  const minutes = el(document_, 'span', { class: 'clk-minutes' });
  const seconds = el(document_, 'small', { class: 'clk-seconds' });
  const period = el(document_, 'em', { class: 'clk-period' });
  const date = el(document_, 'span', {});
  const zoneLabel = el(document_, 'strong', {});
  const readout = el(document_, 'div', { class: 'clk-readout', role: 'timer', 'aria-live': 'off' },
    hours, el(document_, 'b', { class: 'clk-colon', 'aria-hidden': 'true' }, ':'), minutes, seconds, period);
  const zone = el(document_, 'select', { name: 'zone', 'aria-label': 'Time zone' },
    ...ZONES.map(([id, name]) => el(document_, 'option', { value: id }, name)));
  const format = el(document_, 'select', { name: 'format', 'aria-label': 'Display' },
    el(document_, 'option', { value: '12' }, '12 hour'), el(document_, 'option', { value: '24' }, '24 hour'));
  const query = el(document_, 'form', { class: 'wx-query clk-query', 'aria-label': 'Change clock display' },
    el(document_, 'label', { class: 'wx-grow' }, el(document_, 'span', {}, 'Time zone'), zone),
    el(document_, 'label', {}, el(document_, 'span', {}, 'Display'), format));
  query.addEventListener('keydown', (event) => event.stopPropagation());
  const change = () => {
    const state = widget.state as ClockState;
    widget.apply({ ...state, zone: zone.value, hour12: format.value !== '24' });
  };
  zone.addEventListener('change', change);
  format.addEventListener('change', change);
  query.addEventListener('submit', (event) => event.preventDefault());
  widget.body.append(
    query,
    el(document_, 'div', { class: 'clk-case' },
      el(document_, 'div', { class: 'clk-case-top', 'aria-hidden': 'true' }, el(document_, 'span', {}, 'FORMA'), el(document_, 'i', {}, 'QUARTZ · DIGITAL')),
      el(document_, 'div', { class: 'clk-glass' }, readout, el(document_, 'div', { class: 'clk-meta' }, date, zoneLabel)),
      el(document_, 'div', { class: 'clk-controls', 'aria-hidden': 'true' }, el(document_, 'span', {}, 'SNOOZE'), el(document_, 'span', {}, 'TIME SET'), el(document_, 'span', {}, 'ALARM'))),
  );
  const tick = () => {
    const state = widget.state as ClockState;
    let shown: Parts;
    try {
      shown = clockParts(new Date(), state.zone, state.hour12);
    } catch {
      shown = clockParts(new Date(), 'local', state.hour12);
    }
    hours.textContent = shown.hours;
    minutes.textContent = shown.minutes;
    seconds.textContent = shown.seconds;
    period.textContent = shown.period;
    date.textContent = shown.date;
    zoneLabel.textContent = zoneName(state.zone).toUpperCase();
    readout.setAttribute('aria-label', `Time in ${zoneName(state.zone)}: ${shown.hours}:${shown.minutes} ${shown.period}`.trim());
  };
  widget.onState((next) => {
    const state = next as ClockState;
    zone.value = state.zone;
    format.value = state.hour12 ? '12' : '24';
    tick();
  });
  zone.value = (widget.state as ClockState).zone;
  format.value = (widget.state as ClockState).hour12 ? '12' : '24';
  tick();
  const timer = window.setInterval(tick, 1000);
  return () => window.clearInterval(timer);
}
