# Thin wrapper over the stock Odoo image: renders odoo.conf from env/secrets
# at boot so credentials never land in the image.
FROM odoo:18

USER root
COPY odoo.conf.template /etc/odoo/odoo.conf.template
COPY entrypoint.sh /usr/local/bin/sp-entrypoint.sh
COPY pregenerate-assets.py /usr/local/lib/sp-pregenerate-assets.py
RUN chmod +x /usr/local/bin/sp-entrypoint.sh \
    && apt-get update && apt-get install -y --no-install-recommends gettext-base \
    && rm -rf /var/lib/apt/lists/*
USER odoo

ENTRYPOINT ["/usr/local/bin/sp-entrypoint.sh"]
