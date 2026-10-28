import { App } from 'aws-cdk-lib';
import { GenuiFoundation } from './stack.js';
const app = new App();
// Environment-agnostic synthesis requires no account lookup or credential.
new GenuiFoundation(app, 'GenuiSyntheticFoundation');
