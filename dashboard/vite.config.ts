import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		proxy: {
			'/tdata': {
				target: 'http://localhost:3467',
				configure: (proxy) => {
					proxy.on('proxyReq', (proxyReq) => {
						proxyReq.setHeader('x-tenant-id', 'default');
						proxyReq.setHeader('x-temper-principal-kind', 'admin');
					});
				}
			},
			'/observe': {
				target: 'http://localhost:3467',
				configure: (proxy) => {
					proxy.on('proxyReq', (proxyReq) => {
						proxyReq.setHeader('x-tenant-id', 'default');
						proxyReq.setHeader('x-temper-principal-kind', 'admin');
					});
				}
			}
		}
	}
});
