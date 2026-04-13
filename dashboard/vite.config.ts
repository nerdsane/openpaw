import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		proxy: {
			'/tdata': {
				target: 'http://localhost:3467'
			},
			'/observe': {
				target: 'http://localhost:3467'
			},
			'/api': {
				target: 'http://localhost:3467'
			},
			'/paw': {
				target: 'http://localhost:3467'
			},
			'/auth': {
				target: 'http://localhost:3467'
			}
		}
	}
});
