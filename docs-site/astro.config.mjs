// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
	integrations: [
		starlight({
			title: 'VASSL Docs',
			description: 'Documentation for VASSL — Kamalu Ltd internal operations platform.',
			logo: {
				src: './src/assets/vassl-logo.png',
				alt: 'VASSL',
			},
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/kamalu-ltd/vassl' },
			],
			editLink: {
				baseUrl: 'https://github.com/kamalu-ltd/vassl/edit/main/docs/',
			},
			customCss: ['./src/styles/custom.css'],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'What is VASSL?', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'First Run', slug: 'getting-started/first-run' },
					],
				},
				{
					label: 'Modules',
					items: [
						{ label: 'Inventory', slug: 'modules/inventory' },
						{ label: 'Price Book', slug: 'modules/pricebook' },
						{ label: 'Quotations', slug: 'modules/quotations' },
						{ label: 'Suppliers', slug: 'modules/suppliers' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Keyboard Shortcuts', slug: 'reference/keybindings' },
						{ label: 'Settings', slug: 'reference/settings' },
						{ label: 'Global Search', slug: 'reference/global-search' },
						{ label: 'Audit Log', slug: 'reference/audit-log' },
					],
				},
				{
					label: 'Release Notes',
					items: [
						{ label: 'v0.1.0', slug: 'releases/v0-1-0' },
					],
				},
			],
		}),
	],
});
