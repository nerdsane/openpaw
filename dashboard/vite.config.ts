import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		proxy: {
			'/tdata': 'http://localhost:3467',
			'/observe': 'http://localhost:3467'
		}
	}
});
