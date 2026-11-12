function resourceUrl(name) {
  return import.meta.env.BASE_URL + name.split('/').map(encodeURIComponent).join('/');
}

export async function loadImage(name) {
  const image = new Image();
  image.src = resourceUrl(name);
  try {
    await image.decode();
    return image;
  } catch {
    throw new Error(`Не удалось загрузить атлас ${name}`);
  }
}

export async function loadLayer(name) {
  try {
    const response = await fetch(resourceUrl(name));
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    return { name, text: await response.text() };
  } catch (error) {
    throw new Error(`${name}: ${error.message}`);
  }
}
