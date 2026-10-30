// APP-103: the weather widget, ported from our own forma-generative-ui
// `registry/artifacts/weather-card/1.1.0` (its look and structure; there it
// was FormaJS declarative attributes in a shadow-DOM host, here a plain
// card). Conditions come from `GET /v1/widgets/weather` (Open-Meteo, or the
// labeled fixture, APP-098 DS-65). Every value is set as text.
import { el } from '../render';
import { type WidgetCard } from './card';
import { placeOf, type WeatherState } from './state';

export const WEATHER_URL = '/v1/widgets/weather';

type Conditions = {
  location: string;
  temperature: string;
  apparent: string;
  condition: string;
  icon: string;
  wind: string;
  humidity: string;
  precipitation: string;
  units: string;
  provider_label: string;
  source: string;
};
const FIELDS: readonly (keyof Conditions)[] = [
  'location', 'temperature', 'apparent', 'condition', 'icon', 'wind', 'humidity', 'precipitation', 'units', 'provider_label', 'source',
];
const ICONS = ['clear', 'cloudy', 'overcast', 'fog', 'rain', 'snow', 'storm'];

function conditionsOf(value: unknown): Conditions | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  const out: Partial<Conditions> = {};
  for (const field of FIELDS) {
    const text = record[field];
    if (typeof text !== 'string' || text.length > 120) return null;
    out[field] = text;
  }
  return out as Conditions;
}

export function mountWeather(widget: WidgetCard): void {
  const document_ = widget.card.ownerDocument;
  const text = (cls: string) => el(document_, 'strong', { class: cls });
  const temperature = el(document_, 'strong', { class: 'wx-temp' }, '—');
  const condition = el(document_, 'span', { class: 'wx-condition' }, 'Loading conditions…');
  const orb = el(document_, 'span', { class: 'wx-orb', 'data-icon': 'clear', 'aria-hidden': 'true' }, el(document_, 'span', {}));
  const humidity = text('wx-value');
  const precipitation = text('wx-value');
  const wind = text('wx-value');
  const place = el(document_, 'span', { class: 'wx-place' });
  const apparent = el(document_, 'span', { class: 'wx-apparent' });
  const source = el(document_, 'span', { class: 'badge wg-source' }, '…');
  widget.chips.prepend(source);
  const point = (label: string, value: HTMLElement) =>
    el(document_, 'div', { class: 'wx-point' }, el(document_, 'span', {}, label), value);
  // The ported query form: a place and units.
  const location = el(document_, 'input', { name: 'location', value: '', minlength: '2', maxlength: '80', autocomplete: 'off', required: '', 'aria-label': 'Location' });
  const units = el(document_, 'select', { name: 'units', 'aria-label': 'Units' },
    el(document_, 'option', { value: 'imperial' }, '°F'), el(document_, 'option', { value: 'metric' }, '°C'));
  const refresh = el(document_, 'button', { type: 'submit' }, 'Update');
  const query = el(document_, 'form', { class: 'wx-query', 'aria-label': 'Change weather location' },
    el(document_, 'label', { class: 'wx-grow' }, el(document_, 'span', {}, 'Location'), location),
    el(document_, 'label', {}, el(document_, 'span', {}, 'Units'), units),
    refresh);
  query.addEventListener('keydown', (event) => event.stopPropagation());
  widget.body.append(
    query,
    el(document_, 'div', { class: 'wx-hero' }, el(document_, 'div', { class: 'wx-hero-text' }, temperature, condition), orb),
    el(document_, 'div', { class: 'wx-grid' }, point('Humidity', humidity), point('Precip.', precipitation), point('Wind', wind)),
    el(document_, 'p', { class: 'wx-support' }, place, ' · ', apparent),
  );
  let request = 0;
  const load = async (state: WeatherState) => {
    const mine = ++request;
    location.value = state.location;
    units.value = state.units;
    widget.card.dataset.loading = 'true';
    const url = `${WEATHER_URL}?${new URLSearchParams({ location: state.location, units: state.units }).toString()}`;
    let body: unknown = null;
    let status = 0;
    try {
      const response = await fetch(url, { credentials: 'same-origin', cache: 'no-store' });
      status = response.status;
      body = await response.json().catch(() => null);
    } catch {
      status = 0;
    }
    if (mine !== request) return;
    delete widget.card.dataset.loading;
    const conditions = status === 200 ? conditionsOf(body) : null;
    if (!conditions) {
      const code = typeof body === 'object' && body !== null ? (body as { code?: unknown }).code : '';
      condition.textContent = code === 'location_not_found'
        ? `No place called “${state.location}” was found.`
        : 'Conditions are not available right now.';
      widget.card.dataset.state = 'failed';
      return;
    }
    widget.card.dataset.state = 'ready';
    temperature.textContent = conditions.temperature;
    condition.textContent = conditions.condition;
    orb.dataset.icon = ICONS.includes(conditions.icon) ? conditions.icon : 'cloudy';
    humidity.textContent = conditions.humidity;
    precipitation.textContent = conditions.precipitation;
    wind.textContent = conditions.wind;
    place.textContent = conditions.location;
    apparent.textContent = conditions.apparent;
    const live = conditions.source === 'live';
    source.textContent = live ? 'Live · Open-Meteo' : 'Sample conditions';
    source.dataset.source = live ? 'live' : 'fixture';
    source.title = live ? 'Current conditions from Open-Meteo' : 'Canned conditions (demo or offline); not a live reading';
  };
  query.addEventListener('submit', (event) => {
    event.preventDefault();
    const name = placeOf(location.value);
    if (!name) {
      widget.announce('Give a place name of 2 to 80 letters.');
      return;
    }
    const state = widget.state as WeatherState;
    widget.apply({ ...state, location: name, units: units.value === 'metric' ? 'metric' : 'imperial' });
  });
  widget.onState((next, previous) => {
    const now = next as WeatherState;
    const before = previous as WeatherState;
    if (now.location !== before.location || now.units !== before.units) void load(now);
  });
  void load(widget.state as WeatherState);
}
