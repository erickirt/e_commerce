from typing import Optional

from blacksheep import FromJSON, Response
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import created, ok, status_code

from . import router
from .dto import (
    TagCreateReqDto,
    TagUpdateReqDto,
    TagNodeDto,
    TagUpdateRespDto,
    TagReadRespDto,
)


class TagController(APIController):
    @router.post("/tag")
    async def create(self, reqbody: FromJSON[TagCreateReqDto]) -> Response:
        reqbody = reqbody.value
        tag_d = TagUpdateRespDto(
            node=TagNodeDto(name=reqbody.name, id_=12345),
            parent=reqbody.parent,
        )
        return created(message=tag_d.model_dump())

    @router.patch("/tag/{t_id}")
    async def modify(self, t_id: int, reqbody: FromJSON[TagUpdateReqDto]) -> Response:
        reqbody = reqbody.value
        tag_d = TagUpdateRespDto(
            node=TagNodeDto(name=reqbody.name, id_=t_id),
            parent=reqbody.parent,
        )
        return ok(message=tag_d.model_dump())

    @router.delete("/tag/{t_id}")
    async def remove(self, t_id: int) -> Response:
        return status_code(204, "\n")

    @router.get("/tag/{t_id}")
    async def get_tag(
        self, t_id: int, acs: Optional[int], desc_lvl: Optional[int]
    ) -> Response:
        ancestors = None
        if acs:
            ancestors = [TagNodeDto(name="fake-ancestor", id_=1023)]
        descendants = None
        if desc_lvl:
            descendants = [TagNodeDto(name="fake-descendent", id_=1025)]
        tag_d = TagReadRespDto(
            curr_node=TagNodeDto(name="todo-load-this", id_=t_id),
            ancestors=ancestors,
            descendants=descendants,
        )
        return ok(message=tag_d.model_dump())
