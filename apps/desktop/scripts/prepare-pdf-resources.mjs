import path from 'node:path';
import { preparePdfResources } from '../../../scripts/desktop/pdf-resources.mjs';

if (!process.argv[2]) throw new Error('Provide the PDF runtime node_modules output directory.');
preparePdfResources(path.resolve(process.argv[2]));
