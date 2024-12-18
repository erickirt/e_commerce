"""add-currency-column

Revision ID: 000002
Revises: 000001
Create Date: 2024-07-01 16:14:17.405076

"""

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision = "000002"
down_revision = "000001"
branch_labels = None
depends_on = "000001"


def upgrade():
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column(
        "store_profile",
        sa.Column(
            "currency",
            sa.Enum("TWD", "INR", "IDR", "THB", "USD", name="storecurrency"),
            nullable=False,
        ),
    )
    # ### end Alembic commands ###


def downgrade():
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_column("store_profile", "currency")
    # ### end Alembic commands ###
