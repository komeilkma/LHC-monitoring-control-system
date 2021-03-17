<?php 
include("includes/controller.php");
$pagename = 'usergroups';
$container = '';
if(!$session->isAdmin()){
    header("Location: ".$configs->homePage());
    exit;
}
else{
?>
<!DOCTYPE html>
<html>
    <head>
       <title>IPM Software</title>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">

        <link href="css/bootstrap.min.css" rel="stylesheet">
        <link href="fonts/Open Iconic/css/open-iconic-bootstrap.min.css" rel="stylesheet">
        <link href="fonts/font-awesome/css/font-awesome.min.css" rel="stylesheet">

        <link href="css/navigation.css" rel="stylesheet">
        <link href="css/style.css" rel="stylesheet">
        <link href="css/animation.css" rel="stylesheet">        
        
        <!-- Datatables CSS -->
        <link href="css/plugins/datatables/dataTables.bootstrap.min.css" rel="stylesheet">
        
        <!-- Chosen CSS -->
        <link href="css/plugins/chosen/chosen.css" rel="stylesheet">
        
    </head>
    <body>
        <!-- Page Wrapper -->
        <div id="page-wrapper">

            <!-- Side Menu -->
            <nav id="side-menu" class="navbar-default navbar-static-side" role="navigation">
                <div id="sidebar-collapse">
                    <div id="logo-element">
                        <a class="logo" href="index.php">
                               <img src="logo2.png">
                        </a>
                    </div>
                    <?php include('navigation.php'); ?>
                </div>
            </nav>
            <!-- END Side Menu -->

            <?php include('top-navbar.php'); ?>      

            <!-- Page Content -->
            <div id="page-content" class="gray-bg">

                <!-- Title Header -->
                <div class="title-header white-bg">
                    <i class="oi oi-star"></i>
                    <h2>User Groups</h2>
                    <ol class="breadcrumb">
                        <li>
                            <a href="index.php">Home</a>
                        </li>
                        <li class="active">
                            User Groups
                        </li>
                    </ol>
                </div>
                <!-- END Title Header -->
                
                <div class="row">                                     
                    <div class="col-sm-12 col-md-12">
                        <div class="panel">
                            <div class="panel-body">
                                <h4><strong>User Groups</strong> - Create, view and edit user groups. Assign users to user groups. </h4>
                            </div>
                        </div>
                    </div>                                     
                </div>
             
                <div class="row">
                    <div class="col-sm-4 col-md-3 col-lg-2">
                        <div class="panel">
                            <div class="panel-body">
                                <button href="#createGroup" type="button" class="btn btn-main" data-toggle="modal">Create Group</button>
                            </div>
                        </div>
                    </div>
                    <div class="col-sm-8 col-md-9 col-lg-10">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">User Groups</h2>
                            </div>
                            <div class="panel-body">
                                    <table class="table table-striped table-bordered table-hover" id="dataTable1">
                                        <thead>
                                            <tr>
                                                <th>Group Name</th>
                                                <th>Group Level</th>
                                                <th># of Members</th>
                                                <th class='text-center'>Actions</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            <?php
                                            $sql = "SELECT * FROM `groups` WHERE group_id != '1'";
                                            $result = $db->prepare($sql);
                                            $result->execute();
                                            $stop = $adminfunctions->createStop($session->username, 'delete-group');
                                            // If SuperAdmin allow viewing of Administrators Group
                                            if ($session->isSuperAdmin()) {
                                                echo "<tr><td>Administrators</td><td>1</td><td>" . $functions->checkGroupNumbers($db, 1) . "</td>";
                                                echo "<td class='text-center'><div class='btn-group btn-group-xs'><a href='#' data-logid='1' data-target='#editGroups' data-toggle='modal' title='Edit' class='open_modal btn btn-default'><i class='fa fa-pencil'></i></a></td></tr>";
                                            }
                                            while ($row = $result->fetch()) {
                                                echo "<tr><td>" . $row['group_name'] . "</td><td>" . $row['group_level'] . "</td><td>" . $functions->checkGroupNumbers($db, $row['group_id']) . "</td>";
                                                echo "<td class='text-center'><div class='btn-group btn-group-xs'><a href='#' data-logid='" . $row['group_id'] . "' data-target='#editGroups' data-toggle='modal' title='Edit' class='open_modal btn btn-default'><i class='fa fa-pencil'></i></a>";
                                                echo "<a href='includes/adminprocess.php?delete=" . $row['group_id'] . "&stop=" . $stop . "&form_submission=delete_group' title='Delete' class='btn btn-danger confirmation'><i class='fa fa-times'></i></a></div></td></tr>";
                                            }
                                            ?>
                                        </tbody>
                                    </table>
                            </div>
                        </div>                    
                    </div>                                     
                </div>
                <!-- END Row -->
                
                <!--  Modals -->
                <div class="modal fade" id="createGroup" tabindex="-1" role="dialog" aria-labelledby="createGroup" aria-hidden="true">
                    <div class="modal-dialog">
                        <div class="modal-content" id="modal-content">
                            <form action="includes/adminprocess.php" id="user-groups-create" class="form-horizontal" method="post">
                                <div class="modal-header">
                                    <button type="button" class="close" data-dismiss="modal" aria-hidden="true">&times;</button>
                                    <h4 class="modal-title" id="myModalLabel">Create a New Group</h4>
                                </div>
                                <div class="modal-body" id="modal-body">                                   
                                    <div class="form-group">
                                        <label for="group_name" class="col-sm-4 control-label">New Group Name : </label>
                                        <div class="col-md-8">
                                            <input type="text" id="group_name" name="group_name" class="form-control" placeholder="Group Name" />
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <label for="sitedesc" class="col-sm-4 control-label">Assign Group Level : </label>
                                        <div class="col-md-8">
                                            <input type="text" name="group_level" class="form-control" placeholder="Group Level - Enter a number between 2 - 256" data-toggle="tooltip" data-placement="bottom" title="A Group Level is another means of protecting content. For example, protect pages from those in groups lower than level 5." />
                                        </div>
                                    </div>                                       
                                </div>
                                <div class="modal-footer">
                                    <input type="hidden" name="form_submission" value="group_creation">  
                                    <button type="button" class="btn btn-default" data-dismiss="modal">Close</button>
                                    <button type="submit" class="btn btn-primary" id="submit">Create Group</button>
                                </div>
                            </form>
                        </div>
                    </div>
                </div>

                <div class="modal fade" class="modal" id="editGroups" tabindex="-1" role="dialog" aria-labelledby="editGroups" aria-hidden="true">
                    <div class="modal-dialog" id="modal-info">
                        <!-- Content is dynamically pulled from editgroup.php -->
                    </div>
                </div>
                <!-- End Modals-->

              
                <footer>Copyright &copy; <?php echo date("Y"); ?> <a href="http://ipm.ir" target="_blank">IPM</a> - All rights reserved. </footer>

            </div>
            <!-- END Page Content -->

            <?php include('rightsidebar.php'); ?>

        </div>
        <!-- END Page Wrapper -->
        
        <!-- Scroll to top -->
        <a href="#" id="to-top" class="to-top"><i class="oi oi-chevron-top"></i></a>

        <!-- JavaScript Resources -->
        <script src="js/jquery-2.1.3.min.js"></script>
        <script src="js/bootstrap.min.js"></script>
        <script src="js/plugins/metisMenu/jquery.metisMenu.js"></script>
        <script src="js/komeil.js"></script>
        
        <!-- Initialize Form Validation -->
        <script src="js/plugins/formValidation/userGroupsFormsValidation.js"></script>
        <script src="js/plugins/formValidation/jquery.validate.js"></script>
        <script>$(function() { FormsValidation.init(); });</script> 
        
        <script type="text/javascript">
            $('.confirmation').on('click', function () {
                return confirm('Are you sure you wish to delete this group? It will remove all users from the group.');
            });
        </script>
        
        <!-- This dynamically loads the Modal with the Group Info -->
        <script>
            $(document).on("click", ".open_modal", function () {
                var group_id = $(this).data('logid');
                $("#modal-info").load("editgroup.php?log_id=" + group_id);
            });
        </script>

    </body>
</html>
<?php
}
?>