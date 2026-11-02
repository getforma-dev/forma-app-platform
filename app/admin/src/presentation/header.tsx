import { h, createList, createShow } from '@getforma/core';
import type { PresentationProps } from './presentation';
/** Native fallback leaves every admitted component inline and reachable. */
export function PresentationHeader(props: { presentation: PresentationProps }) {
  return <div data-app-presentation-header="">
    {createShow(() => props.presentation.hasDescription, () => <p class="app-description">{props.presentation.description}</p>)}
    <div class="app-actions">{createList(() => props.presentation.actions, action => action.id, action => <button type="button" class="app-action" data-app-action={action.id}>{action.label}</button>)}</div>
  </div>;
}
