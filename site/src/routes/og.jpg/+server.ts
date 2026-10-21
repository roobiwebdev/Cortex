import shareImage from '#lib/assets/og.jpg';

export const prerender = false;

// The share image used to be static/og.jpg. Old links land here and go on to the current image.
// Its URL changes with each new image, so the redirect isn't cached.
export function GET() {
	return new Response(null, {
		status: 302,
		headers: { location: shareImage, 'cache-control': 'no-store' }
	});
}
