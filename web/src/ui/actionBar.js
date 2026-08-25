export class ActionBar {
    constructor() {
        this.holder = document.createElement("div");
        this.holder.style.position = "absolute";
        this.holder.style.bottom = "10px";
        this.holder.style.left = "50%";
        this.holder.style.transform = "translate(-50%, -10px)";
        this.holder.style.height = 67;
        this.holder.style.background = "rgba(0, 0, 0, 0.2)";
        this.holder.style.backdropFilter = "blur(2px)";
        this.holder.style.borderRadius = "4px";
        this.holder.style.display = "flex";
        this.holder.style.alignItems = "center";
        this.holder.style.justifyContent = "center";
        this.holder.style.gap = "8px";

        this.children = [];

        document.body.appendChild(this.holder);
    }

    addChild(id, textures) {
        const child = document.createElement("canvas");
        child.width = child.height = 67;
        const c = child.getContext("2d");

        for (let i = 0; i < textures.length; i++) {
            const img = new Image();
            img.src = textures[i];

            img.onload = () => {
                switch (id) {
                    case 0: {
                        c.drawImage(img, 10, 0 + (23.125 * i), 50, 50);
                        // c.drawImage(img, 10, 25, 50, 50);
                        break;
                    }
                    default: c.drawImage(img, 0, 0, 67, 67);
                }
            }
        }

        // img.onload = () => c.drawImage(img, 0, 0, 67, 67);

        this.holder.appendChild(child);
        this.children.push(child);
        return c;
    }

    update(weapons) {
        for (const child of this.children) {
            this.holder.removeChild(child);
        }

        for (const weapon of weapons) {
            this.addChild(weapon.id, weapon.textures);
        }
    }
}