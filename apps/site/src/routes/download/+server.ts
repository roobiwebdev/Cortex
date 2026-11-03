import { DOWNLOAD_URL } from '#lib/site.ts';

export const prerender = false;

export function GET() {
	return new Response(null, {
		status: 302,
		headers: { location: DOWNLOAD_URL, 'cache-control': 'no-store' }
	});
}
